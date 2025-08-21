// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! McuBoot Ping Packet Implementation
//!
//! This module provides structures and functionality for handling McuBoot ping packets.
//! Ping packets are used to verify communication with the target device and retrieve
//! basic information about the bootloader version and capabilities.
//!
//! The ping mechanism is a simple request-response protocol where the host sends
//! a ping packet and the target responds with version and options information.
//! This is the first communication performed when establishing a
//! connection with a McuBoot-enabled device.

use crate::{CommunicationError, mboot::ResultComm};

use super::{Packet, PacketParse};

/// McuBoot ping response packet structure
///
/// Represents the response received from a McuBoot device after sending a ping packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingResponse {
    /// Bootloader version
    pub version: u32,

    /// Bootloader options
    pub options: u16,
}

impl PacketParse for PingResponse {
    /// Parses raw bytes into a [`PingResponse`] packet
    ///
    /// This method extracts the version and options information from the received
    /// ping response bytes. The parsing follows the McuBoot protocol specification
    /// for ping response packet format.
    ///
    /// # Arguments
    /// * `bytes` - Raw response bytes (without protocol header)
    ///
    /// # Returns
    /// A Result containing the parsed [`PingResponse`]
    ///
    /// # Packet Format
    /// The ping response payload has the following structure:
    /// - Bytes 0-1: Reserved/unused
    /// - Bytes 2-5: Version (4 bytes, big-endian)
    /// - Bytes 6-7: Options (2 bytes, little-endian)
    fn parse(bytes: &[u8]) -> ResultComm<PingResponse> {
        Ok(PingResponse {
            version: u32::from_be_bytes(bytes[2..6].try_into().or(Err(CommunicationError::InvalidData))?),
            options: u16::from_le_bytes(bytes[6..8].try_into().or(Err(CommunicationError::InvalidData))?),
        })
    }
}

impl Packet for PingResponse {
    /// Returns the packet type identifier for ping response packets (0xA7)
    fn get_code() -> u8 {
        super::PINGR
    }
}

/// McuBoot ping packet structure
///
/// Represents a ping packet that can be sent to a McuBoot device to
/// verify communication and request basic information. The ping packet has
/// no payload data - it's simply a request for the target to respond with
/// its version and capabilities.
///
/// # Usage
/// Ping packets are sent as the first communication when establishing
/// a connection with a target device. They serve as both a connectivity test
/// and a way to retrieve basic device information.
pub struct Ping;

impl Packet for Ping {
    /// Returns the packet type identifier for ping packets (0xA6)
    fn get_code() -> u8 {
        super::PING
    }
}
