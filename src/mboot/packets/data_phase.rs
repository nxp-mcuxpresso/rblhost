// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! McuBoot Data Phase Packet Implementation
//!
//! This module provides structures and functionality for handling McuBoot data phase packets.
//! Data phase packets are used to transmit additional data after a command packet when the
//! command requires it (indicated by the [`CommandFlag::HasDataPhase`] flag in the command header).
//!
//! Data phase packets are typically used with commands like:
//! - [`CommandTag::WriteMemory`]: Contains the actual data to be written to memory
//! - [`CommandTag::ReceiveSBFile`]: Contains the SB (Secure Boot) file data
//! - [`CommandTag::ConfigureMemory`]: Contains configuration data for memory setup

#[cfg(doc)]
use crate::tags::{command::CommandTag, command_flag::CommandFlag};

use crate::mboot::ResultComm;

use super::{Packet, PacketConstruct, PacketParse, construct_header};

/// Data phase packet identifier as defined by McuBoot protocol
const DATA_PHASE_CODE: u8 = 0xA5;

/// McuBoot data phase packet structure
///
/// Represents a data phase packet that carries additional data for commands that require it.
/// The data phase packet is sent after the command packet when the command's
/// [`CommandFlag::HasDataPhase`]
/// flag is set. This allows for transmission of variable-length data that exceeds the
/// command packet's parameter capacity.
///
/// # Usage
/// If using the McuBoot high-level interface, data phase packets are automatically sent
/// when a command requires additional data. However, if you're working with command packets
/// directly, you'll need to create and send the corresponding data phase packet manually
/// for commands that have the [`CommandFlag::HasDataPhase`] flag set.
pub struct DataPhasePacket {
    /// Raw data payload to be transmitted
    pub data: Vec<u8>,
}

impl Packet for DataPhasePacket {
    /// Returns the packet type identifier for data phase packets (0xA5)
    fn get_code() -> u8 {
        DATA_PHASE_CODE
    }
}

impl PacketConstruct for DataPhasePacket {
    /// Constructs a complete data phase packet ready for transmission
    ///
    /// This method wraps the data payload with the appropriate McuBoot protocol
    /// header to create a properly formatted data phase packet. The header includes
    /// the packet type, length, and CRC checksum as required by the protocol.
    ///
    /// # Returns
    /// A [`Vec<u8>`] containing the complete data phase packet with protocol header:
    /// - Start byte (0x5A)
    /// - Packet code (0xA5)
    /// - Length (2 bytes, little-endian)
    /// - CRC16 (2 bytes, little-endian)
    /// - Data payload (variable length)
    fn construct(&self) -> Vec<u8> {
        construct_header(DATA_PHASE_CODE, self.data.clone())
    }
}

impl PacketParse for DataPhasePacket {
    /// Creates a [`DataPhasePacket`] from raw bytes
    ///
    /// # Arguments
    /// * `bytes` - Raw data bytes to be stored in the packet
    ///
    /// # Returns
    /// A Result containing the new [`DataPhasePacket`] with the provided data
    ///
    /// # Note
    /// This method always succeeds as data phase packets can contain any
    /// arbitrary byte sequence as their payload.
    fn parse(bytes: &[u8]) -> ResultComm<DataPhasePacket> {
        Ok(DataPhasePacket { data: bytes.to_vec() })
    }
}
