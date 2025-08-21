// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! McuBoot Command Packet Implementation
//!
//! This module provides structures and functionality for creating and handling McuBoot command packets.
//! McuBoot is NXP's bootloader protocol used for communication with target devices during firmware
//! updates and debugging operations.

use crate::mboot::tags::{
    ToAddress, command::CommandTag, command_flag::CommandFlag, command_response::CmdResponseTag, status::StatusCode,
};

use super::{Packet, construct_header};

/// Command packet header structure
///
/// Contains the command flag indicating whether the command has a data phase,
/// and a reserved byte for future use. This structure is part of every McuBoot
/// command packet sent to the target device.
#[derive(Clone, Debug)]
pub struct CommandHeader {
    /// Flag indicating command characteristics (e.g., has data phase)
    pub flag: CommandFlag,
    /// Reserved byte for future protocol extensions
    pub reserved: u8,
}

impl CommandHeader {
    /// Constructs a complete command frame ready for transmission
    ///
    /// This method builds a McuBoot command packet by combining the command code,
    /// flags, parameter count, and parameters into a properly formatted frame.
    /// The frame includes the protocol header and follows the McuBoot packet format.
    ///
    /// # Arguments
    /// * `params` - Slice of `u32` parameters for the command
    /// * `command_code` - The command identifier (from [`CommandTag`] enum)
    ///
    /// # Returns
    /// A [`Vec<u8>`] containing the complete command frame ready for transmission
    ///
    /// # Frame Format
    /// The constructed frame follows this structure:
    /// - Protocol header (added by `construct_header`)
    /// - Command code (1 byte)
    /// - Command flag (1 byte)  
    /// - Reserved byte (1 byte)
    /// - Parameter count (1 byte)
    /// - Parameters (4 bytes each, little-endian)
    #[must_use]
    pub fn construct_frame(&self, params: &[u32], command_code: u8) -> Vec<u8> {
        let mut command_part = vec![command_code, self.flag.code(), self.reserved, params.len() as u8];

        // Pre-allocate space for parameters to avoid multiple allocations
        // Each u32 parameter takes 4 bytes when serialized
        command_part.reserve(params.len() * 4);

        // Convert u32 parameters to little-endian bytes and append to command
        // This matches the McuBoot protocol requirement for little-endian parameter encoding
        command_part.extend(params.iter().flat_map(|num| num.to_le_bytes()));

        construct_header(super::CMD, command_part)
    }
}

/// McuBoot command packet structure
///
/// Represents a complete command packet that can be sent to a McuBoot device.
/// This structure combines the command header with the specific command tag and parameters.
/// Commands can be used for various operations like reading memory, writing flash, getting
/// device properties, etc.
#[derive(Clone, Debug)]
pub struct CommandPacket<'a> {
    /// Command header containing flags and metadata
    pub header: CommandHeader,
    /// Specific command tag with associated parameters
    pub tag: CommandTag<'a>,
}

/// McuBoot command response structure
///
/// Represents the response received from a McuBoot device after sending a command.
/// All McuBoot commands return a response indicating success/failure and may include
/// additional data depending on the command type.
#[derive(Clone, Debug)]
pub struct CmdResponse {
    /// Response header (same format as command header)
    pub header: CommandHeader,
    /// Status code indicating command execution result
    pub status: StatusCode,
    /// Response-specific data (varies by command type)
    pub tag: CmdResponseTag,
}

impl Packet for CommandPacket<'_> {
    fn get_code() -> u8 {
        super::CMD
    }
}

impl<'a> CommandPacket<'a> {
    /// Creates a new command packet without data phase
    ///
    /// Used for commands that don't require additional data to be sent after the command packet.
    /// Examples include [`CommandTag::GetProperty`], [`CommandTag::Reset`], [`CommandTag::Execute`] commands.
    ///
    /// # Arguments
    /// * `tag` - The command tag specifying the operation and parameters
    ///
    /// # Returns
    /// A new [`CommandPacket`] with [`CommandFlag::NoData`] flag set
    #[must_use]
    pub fn new_none_flag(tag: CommandTag<'a>) -> Self {
        CommandPacket {
            header: CommandHeader {
                flag: CommandFlag::NoData,
                reserved: 0,
            },
            tag,
        }
    }

    /// Creates a new command packet with data phase
    ///
    /// Used for commands that require additional data to be sent after the command packet.
    /// Examples include [`CommandTag::WriteMemory`], [`CommandTag::ReceiveSBFile`] commands where the actual data follows
    /// the command packet.
    ///
    /// # Arguments
    /// * `tag` - The command tag specifying the operation and parameters
    ///
    /// # Returns
    /// A new [`CommandPacket`] with [`CommandFlag::HasDataPhase`] flag set
    #[must_use]
    pub fn new_data_phase(tag: CommandTag<'a>) -> Self {
        CommandPacket {
            header: CommandHeader {
                flag: CommandFlag::HasDataPhase,
                reserved: 0,
            },
            tag,
        }
    }
}

impl Packet for CmdResponse {
    fn get_code() -> u8 {
        super::CMD
    }
}

#[cfg(test)]
mod tests {
    use crate::mboot::{
        packets::command::{CommandHeader, CommandPacket},
        tags::{
            ToAddress,
            command::{CommandTag, CommandToParams},
            command_flag::CommandFlag,
            property::PropertyTagDiscriminants,
        },
    };

    fn get_command(tag: CommandTag) -> CommandPacket {
        CommandPacket {
            header: CommandHeader {
                flag: CommandFlag::NoData,
                reserved: 0,
            },
            tag,
        }
    }

    #[test]
    fn test_command_construct_version() {
        let cmd = get_command(CommandTag::GetProperty {
            tag: PropertyTagDiscriminants::CurrentVersion,
            memory_index: 0,
        });
        let bytes = cmd.header.construct_frame(&cmd.tag.to_params().0, cmd.tag.code());
        assert_eq!(
            bytes,
            [
                0x5a, 0xa4, 0xc, 0x0, 0x4b, 0x33, 0x7, 0x0, 0x0, 0x2, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0
            ]
        );
    }

    #[test]
    fn test_command_flash_program_once() {
        let cmd = get_command(CommandTag::FlashProgramOnce {
            index: 0x51,
            count: 4,
            data: 0x12345678,
        });

        let bytes = cmd.header.construct_frame(&cmd.tag.to_params().0, cmd.tag.code());
        assert_eq!(
            bytes,
            [
                90, 164, 16, 0, 27, 96, 14, 0, 0, 3, 81, 0, 0, 0, 4, 0, 0, 0, 120, 86, 52, 18
            ]
        );
    }
}
