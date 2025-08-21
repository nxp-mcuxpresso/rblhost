// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! McuBoot Protocol Abstraction Layer
//!
//! This module provides a unified interface for communicating with McuBoot devices
//! across different physical transports (UART, USB, I2C). It defines common traits and
//! error types that allow the same high-level McuBoot commands to work regardless of the
//! underlying communication medium.
//!
//! The protocol abstraction handles the low-level details of packet framing, acknowledgment
//! handling, and error recovery while providing a consistent interface for sending and
//! receiving McuBoot packets.
//!
//! # Supported Protocols
//! - UART: Serial communication over UART interfaces
//! - USB: USB HID communication for direct device connection
//! - I2C: I2C bus communication for embedded applications

#[cfg(feature = "python")]
use pyo3::{PyErr, exceptions::PyValueError};

use std::time::Duration;

use super::{
    ResultComm,
    packets::{Packet, PacketConstruct, PacketParse},
    tags::status::StatusCode,
};

pub mod i2c;
pub mod uart;
pub mod usb;

/// Communication error types for McuBoot protocol operations
///
/// This enum covers all possible error conditions that can occur during
/// communication with McuBoot devices, from low-level transport errors
/// to protocol-level issues.
#[derive(thiserror::Error, Debug)]
pub enum CommunicationError {
    /// Error from the underlying serial port library
    #[error("error raised by UART library")]
    SerialPortError(#[from] serialport::Error),

    /// General I/O error during read/write operations
    #[error("error occurred while reading or writing to device")]
    IOError(#[from] std::io::Error),

    /// File system error during file operations
    #[error("error while reading or writing a file")]
    FileError(#[source] std::io::Error),

    /// Target device sent a NACK (negative acknowledgment)
    #[error("board sent NACK")]
    NACKSent,

    /// Received packet has incorrect CRC checksum
    #[error("received incorrect CRC")]
    InvalidCrc,

    /// Packet header is malformed or invalid
    #[error("invalid response header")]
    InvalidHeader,

    /// Packet data is invalid or corrupted
    #[error("data in the packet is invalid")]
    InvalidData,

    /// Received unexpected packet type
    #[error("received another packet type than was expected")]
    InvalidPacketReceived,

    /// Error during packet parsing
    #[error("error occured while parsing: {0}")]
    ParseError(String),

    /// Command returned an error status code
    #[error("unexpected status code: {1} ({1:#X}) {0}")]
    UnexpectedStatus(StatusCode, u32),

    /// Communication was aborted by user or system
    #[error("communication was aborted")]
    Aborted,

    /// Feature not supported on current platform
    #[error("this functionality is not supported on the current platform")]
    UnsupportedPlatform,

    /// Timeout occurred while waiting for response
    #[error("timeout occured while waiting for response")]
    Timeout,
}

impl From<StatusCode> for CommunicationError {
    /// Convert a McuBoot status code to a communication error
    fn from(value: StatusCode) -> Self {
        CommunicationError::UnexpectedStatus(value, value.into())
    }
}

#[cfg(feature = "python")]
impl From<CommunicationError> for PyErr {
    /// Convert communication error to Python exception (when Python bindings are enabled)
    fn from(value: CommunicationError) -> Self {
        PyValueError::new_err(value.to_string())
    }
}

/// Core protocol trait for McuBoot communication
///
/// This trait defines the methods that all McuBoot protocol implementations
/// must provide.
#[cfg_attr(any(feature = "python", feature = "c_api"), enum_dispatch::enum_dispatch)]
pub trait Protocol {
    /// Get the configured timeout duration for operations
    fn get_timeout(&self) -> Duration;

    /// Get the polling interval for checking responses
    fn get_polling_interval(&self) -> Duration;

    /// Get a string identifier for this protocol instance
    fn get_identifier(&self) -> &str;

    /// Read raw bytes from the device
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes to read
    ///
    /// # Returns
    /// A Result containing the read bytes or an error
    ///
    /// # Errors
    /// Any errors that occured while reading, from being unable to read to invalid CRC checksum.
    fn read(&mut self, bytes: usize) -> ResultComm<Vec<u8>>;

    /// Write raw packet data to the device
    ///
    /// # Arguments
    /// * `data` - Raw packet bytes to send
    ///
    /// # Returns
    /// A Result indicating success or error
    ///
    /// # Errors
    /// Any errors that occured while writing, from being unable to write to invalid CRC checksum.
    fn write_packet_raw(&mut self, data: &[u8]) -> ResultComm<()>;

    /// Read a raw packet with specific type code
    ///
    /// # Arguments
    /// * `packet_code` - Expected packet type code
    ///
    /// # Returns
    /// A Result containing the packet payload or an error
    ///
    /// # Errors
    /// Any errors that occured while reading, from being unable to read to invalid CRC checksum.
    fn read_packet_raw(&mut self, packet_code: u8) -> ResultComm<Vec<u8>>;

    /// Write a strongly-typed packet to the device
    ///
    /// This method handles packet construction and transmission for any type
    /// that implements the required packet traits.
    ///
    /// # Arguments
    /// * `packet` - The packet to send
    ///
    /// # Returns
    /// A Result indicating success or error
    ///
    /// # Errors
    /// Any errors that occured while writing, from being unable to write to invalid CRC checksum.
    fn write_packet_concrete<T>(&mut self, packet: T) -> ResultComm<()>
    where
        T: PacketConstruct + Packet,
    {
        self.write_packet_raw(&packet.construct())
    }

    /// Read a strongly-typed packet from the device
    ///
    /// This method handles packet reception and parsing for any type
    /// that implements the required packet traits.
    ///
    /// # Returns
    /// A Result containing the parsed packet or an error
    ///
    /// # Errors
    /// Any errors that occured while reading, from being unable to read to invalid CRC checksum.
    fn read_packet_concrete<T>(&mut self) -> ResultComm<T>
    where
        T: PacketParse + Packet,
    {
        let data_slice = self.read_packet_raw(T::get_code())?;
        T::parse(&data_slice)
    }
}

/// Trait for opening protocol connections
pub trait ProtocolOpen: Protocol {
    /// Open a protocol connection with basic identifier
    ///
    /// # Arguments
    /// * `identifier` - Connection identifier (e.g., COM port, device path)
    ///
    /// # Returns
    /// A Result containing the opened protocol instance or an error
    ///
    /// # Errors
    /// Any error raised by the specific protocol library, mostly informing that the selected device does not exist.
    fn open(identifier: &str) -> ResultComm<Self>
    where
        Self: Sized;

    /// Open a protocol connection with advanced options
    ///
    /// # Arguments
    /// * `identifier` - Connection identifier
    /// * `baudrate` - Communication baudrate (protocol-specific)
    /// * `timeout` - Operation timeout duration
    /// * `polling_interval` - Response polling interval
    ///
    /// # Returns
    /// A Result containing the opened protocol instance or an error
    ///
    /// # Errors
    /// Any error raised by the specific protocol library, mostly informing that the selected device does not exist.
    ///
    /// # Note
    /// Default implementation ignores advanced options and calls basic `open()`
    #[expect(
        unused_variables,
        reason = "rust-analyzer would show the underscores for inlay hints"
    )]
    fn open_with_options(
        identifier: &str,
        baudrate: u32,
        timeout: Duration,
        polling_interval: Duration,
    ) -> ResultComm<Self>
    where
        Self: Sized,
    {
        Self::open(identifier)
    }
}

// Define a protocol enum that can be used instead of dyn Protocol
#[cfg(any(feature = "c_api", feature = "python"))]
pub mod protocol_impl;

// Protocol acknowledgment constants as defined by McuBoot specification
/// Positive acknowledgment - command accepted
const ACK: u8 = 0xA1;
/// Negative acknowledgment - command rejected
const NACK: u8 = 0xA2;
/// Abort acknowledgment - operation aborted
const ACK_ABORT: u8 = 0xA3;
