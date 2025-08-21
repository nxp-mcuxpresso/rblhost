// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

//! McuBoot Protocol Packet Implementation
//!
//! The module defines traits for packet construction and parsing, packet type constants,
//! and the common packet header format used across all McuBoot packet types.
//!
//! # Packet Structure
//! All McuBoot packets follow this format:
//! - Start byte (0x5A)
//! - Packet type code (1 byte)
//! - Length (2 bytes, little-endian)
//! - CRC16 (2 bytes, little-endian)
//! - Data payload (variable length)

use super::ResultComm;

pub mod command;
pub mod data_phase;
pub mod ping;

/// Trait for packet type identification
///
/// All packet types must implement this trait to provide their unique identifier
/// as defined by the McuBoot protocol specification.
pub trait Packet {
    /// Returns the packet type code
    #[must_use]
    fn get_code() -> u8;
}

/// Trait for packet construction
///
/// Implemented by packet types that can be serialized into bytes for transmission.
/// The constructed packet includes the complete protocol header and payload.
pub trait PacketConstruct {
    /// Constructs the complete packet as bytes ready for transmission
    #[must_use]
    fn construct(&self) -> Vec<u8>;
}

/// Trait for packet parsing
///
/// Implemented by packet types that can be deserialized from received bytes.
/// The input bytes should contain only the payload data, with the protocol
/// header already processed and removed.
pub trait PacketParse: Sized {
    /// Parses bytes into a packet instance
    ///
    /// # Arguments
    /// * `bytes` - Raw payload bytes (without protocol header)
    ///
    /// # Returns
    /// A Result containing the parsed packet or an error
    ///
    /// # Errors
    /// If `bytes` contains improper data for current packet.
    fn parse(bytes: &[u8]) -> ResultComm<Self>;
}

/// CRC16 calculator using XMODEM polynomial
///
/// Used for packet integrity verification as specified by the McuBoot protocol.
/// All packets include a CRC16 checksum calculated over the header and payload.
pub(super) const CRC_CHECK: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

// McuBoot packet type constants as defined by the protocol specification
#[expect(dead_code, reason = "remove this expect if you have used the variable")]
const ABORT: u8 = 0xA3;
/// Command packet identifier
const CMD: u8 = 0xA4;
#[expect(dead_code, reason = "remove this expect if you have used the variable")]
const DATA: u8 = 0xA5;
/// Ping packet identifier
const PING: u8 = 0xA6;
/// Ping response packet identifier
const PINGR: u8 = 0xA7;

/// Constructs a complete McuBoot packet header with payload
///
/// This function creates a properly formatted McuBoot packet by combining the
/// packet type code, length, CRC checksum, and payload data according to the
/// protocol specification.
///
/// # Arguments
/// * `packet_code` - The packet type identifier (e.g., CMD, DATA, PING)
/// * `data` - The packet payload data
///
/// # Returns
/// A [`Vec<u8>`] containing the complete packet with header and payload
///
/// # Packet Format
/// The constructed packet follows this structure:
/// - Start byte: 0x5A
/// - Packet code: 1 byte
/// - Length: 2 bytes (little-endian, length of data)
/// - CRC16: 2 bytes (little-endian, calculated over header + data)
/// - Data: variable length payload
fn construct_header(packet_code: u8, data: Vec<u8>) -> Vec<u8> {
    let length = data.len() as u16;
    let length = length.to_le_bytes();

    let mut v = vec![0x5A, packet_code, length[0], length[1]];
    v.extend(data);

    let crc = CRC_CHECK.checksum(&v).to_le_bytes();

    v.insert(4, crc[0]);
    v.insert(5, crc[1]);

    v
}
