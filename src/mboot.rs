// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use color_print::cstr;
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, trace};
use packets::{
    Packet, PacketParse,
    command::{CmdResponse, CommandHeader, CommandPacket},
    data_phase::DataPhasePacket,
};
use protocols::Protocol;
use tags::{
    ToAddress,
    command::{CommandTag, CommandToParams, KeyProvOperation, TrustProvOperation},
    command_flag::CommandFlag,
    command_response::CmdResponseTag,
    property::{PropertyTag, PropertyTagDiscriminants},
    status::StatusCode,
};

use crate::CommunicationError;

mod formatters;
pub mod memory;
pub mod packets;
pub mod protocols;
pub mod tags;

/// Response structure for [`CommandTag::GetProperty`] command
///
/// Contains the status code, raw response words, and parsed property value.
#[derive(Clone, Debug)]
pub struct GetPropertyResponse {
    /// Status code of the operation
    pub status: StatusCode,
    /// Raw response words from the device
    pub response_words: Box<[u32]>,
    /// Parsed property value
    pub property: PropertyTag,
}

/// Response structure for [`CommandTag::ReadMemory`] command
///
/// Contains the status code, response metadata, and actual data bytes read.
#[derive(Clone, Debug)]
pub struct ReadMemoryResponse {
    /// Status code of the operation
    pub status: StatusCode,
    /// Response metadata (typically contains the byte count)
    pub response_words: Box<[u32]>,
    /// Actual data bytes read from memory
    pub bytes: Box<[u8]>,
}

/// Response types for [`CommandTag::KeyProvisioning`] operations
#[derive(Clone, Debug)]
pub enum KeyProvisioningResponse {
    /// Simple status response for most key provisioning operations
    Status(StatusCode),
    /// Extended response for [`KeyProvOperation::ReadKeyStore`] operation containing key data
    KeyStore {
        /// Status code of the operation
        status: StatusCode,
        /// Response metadata
        response_words: Box<[u32]>,
        /// Key store data bytes
        bytes: Box<[u8]>,
    },
}

trait InvalidData<T> {
    /// Convert a type to [`Result`] of [`CommunicationError`].
    fn or_invalid(self) -> Result<T, CommunicationError>;
}

impl<T, E> InvalidData<T> for Result<T, E> {
    /// Transforms `E` into [`CommunicationError::InvalidData`]
    fn or_invalid(self) -> Result<T, CommunicationError> {
        self.or(Err(CommunicationError::InvalidData))
    }
}

/// Main MCU Boot communication structure
///
/// Provides high-level interface for bootloader communication over various protocols.
///
/// # Type Parameters
///
/// * `T` - The underlying communication protocol (UART, USB, etc.)
pub struct McuBoot<T>
where
    T: Protocol,
{
    device: T,
    /// Enable/disable progress bar for data transfers
    pub progress_bar: bool,
    pub mask_read_data_phase: bool,
}

/// Result type for communication operations returning a value
pub type ResultComm<T> = Result<T, CommunicationError>;
/// Result type for operations returning only a status code
pub type ResultStatus = ResultComm<StatusCode>;

impl<T> McuBoot<T>
where
    T: Protocol,
{
    /// Creates a new [`McuBoot`] instance with the specified protocol
    ///
    /// # Arguments
    ///
    /// * `device` - The communication protocol instance
    ///
    /// # Returns
    ///
    /// A new [`McuBoot`] instance
    #[must_use]
    pub fn new(device: T) -> Self {
        info!(
            "Initialized MCU Boot with device identifier: {}",
            device.get_identifier()
        );
        McuBoot {
            device,
            progress_bar: false,
            mask_read_data_phase: false,
        }
    }

    /// Get a specific property value from the device
    ///
    /// # Arguments
    ///
    /// * `tag` - The property tag to query
    /// * `memory_index` - External memory ID or internal memory region index
    ///
    /// # Returns
    ///
    /// Property response containing the value and status
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if:
    /// - Communication with device fails
    /// - Invalid response is received
    /// - Property is not supported
    pub fn get_property(
        &mut self,
        tag: PropertyTagDiscriminants,
        memory_index: u32,
    ) -> ResultComm<GetPropertyResponse> {
        let command = CommandPacket::new_none_flag(CommandTag::GetProperty { tag, memory_index });
        self.send_command(&command)?;

        let response = self.read_cmd_response()?;

        if let CmdResponseTag::GetProperty(val) = response.tag {
            Ok(GetPropertyResponse {
                status: response.status,
                property: PropertyTag::from_code(tag, &val),
                response_words: val,
            })
        } else {
            Err(CommunicationError::InvalidPacketReceived)
        }
    }

    /// Set a property value on the device
    ///
    /// # Arguments
    ///
    /// * `tag` - The property tag to set
    /// * `value` - The value to set
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if communication fails
    pub fn set_property(&mut self, tag: PropertyTagDiscriminants, value: u32) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::SetProperty { tag, value });
        self.send_command(&command)?;

        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Reset the MCU
    ///
    /// Sends a reset command to the device. Note that the connection may be lost
    /// after reset and need to be re-established.
    ///
    /// # Returns
    ///
    /// Status code (may be [`StatusCode::NoResponse`] if device resets successfully)
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn reset(&mut self) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::Reset);
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Call a function at the specified address
    ///
    /// # Arguments
    ///
    /// * `start_address` - Function address to call (must be word aligned)
    /// * `argument` - Function argument
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn call(&mut self, start_address: u32, argument: u32) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::Call {
            start_address,
            argument,
        });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Execute program at the specified address
    ///
    /// # Arguments
    ///
    /// * `start_address` - Jump address (must be word aligned)
    /// * `argument` - Function argument address
    /// * `stackpointer` - Stack pointer address
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn execute(&mut self, start_address: u32, argument: u32, stackpointer: u32) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::Execute {
            start_address,
            argument,
            stackpointer,
        });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Fill memory region with a pattern
    ///
    /// # Arguments
    ///
    /// * `start_address` - Start address (must be word aligned)
    /// * `byte_count` - Number of bytes to fill (must be word aligned)
    /// * `pattern` - 32-bit pattern to fill with
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn fill_memory(&mut self, start_address: u32, byte_count: u32, pattern: u32) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::FillMemory {
            start_address,
            byte_count,
            pattern,
        });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Write data to MCU memory
    ///
    /// # Arguments
    ///
    /// * `start_address` - Start address for writing
    /// * `memory_id` - Memory ID (0 for internal memory, see [`memory::mem_id`] for external)
    /// * `bytes` - Data to write
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Note
    ///
    /// Data will be automatically split into chunks based on max packet size
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn write_memory(&mut self, start_address: u32, memory_id: u32, bytes: &[u8]) -> ResultStatus {
        let command = CommandPacket::new_data_phase(CommandTag::WriteMemory {
            start_address,
            memory_id,
            bytes,
        });
        self.send_command(&command)?;

        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Erase all flash memory
    ///
    /// # Arguments
    ///
    /// * `memory_id` - Memory ID (0 for internal flash)
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Warning
    ///
    /// This operation will erase the entire flash memory without recovering
    /// the flash security section.
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn flash_erase_all(&mut self, memory_id: u32) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::FlashEraseAll { memory_id });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Erase a specific flash region
    ///
    /// # Arguments
    ///
    /// * `start_address` - Start address of region to erase
    /// * `byte_count` - Number of bytes to erase
    /// * `memory_id` - Memory ID (0 for internal flash)
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn flash_erase_region(&mut self, start_address: u32, byte_count: u32, memory_id: u32) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::FlashEraseRegion {
            start_address,
            byte_count,
            memory_id,
        });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Erase all flash and recover security section
    ///
    /// This command erases the entire flash memory and recovers the flash security section,
    /// effectively unsecuring the device.
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn flash_erase_all_unsecure(&mut self) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::FlashEraseAllUnsecure);
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Read data from MCU memory
    ///
    /// # Arguments
    ///
    /// * `start_address` - Start address to read from
    /// * `byte_count` - Number of bytes to read
    /// * `memory_id` - Memory ID (0 for internal memory)
    ///
    /// # Returns
    ///
    /// Response containing the read data and status
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if:
    /// - Communication fails
    /// - Invalid response is received
    /// - Memory is protected or inaccessible
    pub fn read_memory(
        &mut self,
        start_address: u32,
        byte_count: u32,
        memory_id: u32,
    ) -> ResultComm<ReadMemoryResponse> {
        let command = CommandPacket::new_none_flag(CommandTag::ReadMemory {
            start_address,
            byte_count,
            memory_id,
        });
        self.send_command(&command)?;

        // allowing one more status code when reading memory
        let response = self.read_command()?;
        let status = &response.status;
        if !(status.is_success() || status.is_memory_blank_page_read_disallowed()) {
            return Err((*status).into());
        }

        if let CmdResponseTag::ReadMemory(bytes) = response.tag {
            Ok(ReadMemoryResponse {
                status: response.status,
                response_words: Box::new([bytes.len() as u32]),
                bytes,
            })
        } else {
            Err(CommunicationError::InvalidPacketReceived)
        }
    }

    /// Configure external memory
    ///
    /// # Arguments
    ///
    /// * `memory_id` - Memory ID to configure
    /// * `address` - Address containing configuration data
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn configure_memory(&mut self, memory_id: u32, address: u32) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::ConfigureMemory { memory_id, address });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Receive and process a Secure Binary (SB) file
    ///
    /// # Arguments
    ///
    /// * `bytes` - SB file data
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Note
    ///
    /// The SB file will be processed and executed by the bootloader.
    /// Progress bar will be shown if enabled.
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn receive_sb_file(&mut self, bytes: &[u8]) -> ResultStatus {
        let command = CommandPacket::new_data_phase(CommandTag::ReceiveSBFile { bytes });
        match self.send_command(&command) {
            Ok(()) | Err(CommunicationError::Aborted) => {
                let response = self.read_cmd_response()?;
                Ok(response.status)
            }
            Err(err) => Err(err),
        }
    }

    /// Execute trust provisioning operation
    ///
    /// Performs various trust provisioning operations on the device, such as
    /// proving genuinity, setting wrapped data, or other security-related operations.
    ///
    /// # Arguments
    ///
    /// * `operation` - The trust provisioning operation to execute
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - Status code indicating success or failure
    /// - Response data specific to the operation
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn trust_provisioning(&mut self, operation: &TrustProvOperation) -> ResultComm<(StatusCode, Box<[u32]>)> {
        let command = CommandPacket::new_none_flag(CommandTag::TrustProvisioning(operation));
        self.send_command(&command)?;

        let response = self.read_cmd_response()?;
        match response.tag {
            CmdResponseTag::TrustProvisioning(data) => Ok((response.status, data)),
            _ => Err(CommunicationError::InvalidPacketReceived),
        }
    }

    /// Execute key provisioning operation
    ///
    /// Handles various key provisioning operations including enrolling PUF,
    /// setting intrinsic keys, writing to non-volatile memory, and managing user keys.
    /// The behavior varies based on the operation type:
    /// - [`KeyProvOperation::SetUserKey`] operations include a data phase for key transmission
    /// - [`KeyProvOperation::ReadKeyStore`] operations return the actual key store data
    /// - Other operations return simple status responses
    ///
    /// # Arguments
    ///
    /// * `operation` - The key provisioning operation to execute
    ///
    /// # Returns
    ///
    /// Response type depends on the operation:
    /// - [`KeyProvisioningResponse::Status`] for most operations
    /// - [`KeyProvisioningResponse::KeyStore`] for [`KeyProvOperation::ReadKeyStore`] with key data
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if:
    /// - Communication with device fails
    /// - Invalid response is received
    /// - Data phase transmission fails for `SetUserKey`
    pub fn key_provisioning(
        &mut self,
        operation: &KeyProvOperation,
    ) -> Result<KeyProvisioningResponse, CommunicationError> {
        let command = CommandPacket::new_none_flag(CommandTag::KeyProvisioning(operation));
        if let KeyProvOperation::ReadKeyStore { .. } = operation {
            self.send_command(&command)?;
            let response = self.read_cmd_response()?;
            // Extract the data based on the response tag
            match response.tag {
                CmdResponseTag::KeyProvisioning(data, data_phase) => {
                    // The data phase should contain the actual key store data
                    Ok(KeyProvisioningResponse::KeyStore {
                        status: response.status,
                        response_words: data,
                        bytes: data_phase.unwrap_or_default(),
                    })
                }
                _ => Err(CommunicationError::InvalidPacketReceived),
            }
        } else {
            self.mask_read_data_phase = true;
            self.send_command(&command)?;
            self.mask_read_data_phase = false;
            let response = self.read_cmd_response()?;
            Ok(KeyProvisioningResponse::Status(response.status))
        }
    }

    /// Read from MCU flash program once region (eFuse/OTP)
    ///
    /// Reads a 32-bit value from the one-time programmable (OTP) memory region.
    /// This memory can only be written once and is typically used for storing
    /// permanent configuration or security keys.
    ///
    /// # Arguments
    ///
    /// * `index` - Start index of the eFuse/OTP region
    /// * `count` - Number of bytes to read (must be 4)
    ///
    /// # Returns
    ///
    /// The read value as a 32-bit unsigned integer
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if:
    /// - Communication with device fails
    /// - Invalid response type is received
    /// - The specified index is out of range
    /// - The OTP region is locked or inaccessible
    pub fn flash_read_once(&mut self, index: u32, count: u32) -> ResultComm<u32> {
        let command = CommandPacket::new_none_flag(CommandTag::FlashReadOnce { index, count });
        self.send_command(&command)?;

        let response = self.read_cmd_response()?;
        match response.tag {
            CmdResponseTag::FlashReadOnce(value) => Ok(value),
            _ => Err(CommunicationError::InvalidPacketReceived),
        }
    }

    /// Write into MCU once program region (eFuse/OTP)
    ///
    /// Programs a 32-bit value into the one-time programmable memory region.
    /// This operation is irreversible - once programmed, the memory cannot be
    /// erased or reprogrammed. In OTP memory, bits can only change from 0 to 1,
    /// never from 1 to 0.
    ///
    /// # Arguments
    ///
    /// * `index` - Start index of the eFuse/OTP region
    /// * `count` - Number of bytes to write (must be 4)
    /// * `data` - 32-bit value to write
    /// * `verify` - If true, reads back and verifies the written value
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure. If verification is enabled
    /// and fails, returns [`StatusCode::OtpVerifyFail`].
    ///
    /// # Notes
    ///
    /// - The verification process checks if all bits that were supposed to be
    ///   set to 1 are actually set. It uses bitwise AND to accommodate the fact
    ///   that some bits might have already been programmed.
    /// - The index is masked to 24 bits during verification read
    /// - ROM might not report errors when attempting to write to locked OTP,
    ///   so verification is recommended for critical operations
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible. Verification fail is not an
    /// error.
    pub fn flash_program_once(&mut self, index: u32, count: u32, data: u32, verify: bool) -> ResultStatus {
        let command = CommandPacket::new_none_flag(CommandTag::FlashProgramOnce { index, count, data });
        self.send_command(&command)?;

        let response = self.read_cmd_response()?;

        if verify && response.status.is_success() {
            // For verification, we read back the value and check if the bits we set are still set
            // Note: In OTP, we can only set bits from 0 to 1, not vice versa
            match self.flash_read_once(index & ((1 << 24) - 1), count) {
                Ok(read_value) => {
                    if read_value & data == data {
                        Ok(response.status)
                    } else {
                        // Custom status code for verification failure
                        Ok(StatusCode::OtpVerifyFail)
                    }
                }
                Err(e) => Err(e),
            }
        } else {
            Ok(response.status)
        }
    }

    /// Read fuse data
    ///
    /// Reads data from the device's fuse memory region. Fuses are one-time
    /// programmable memory bits used for permanent configuration, security
    /// settings, and device-specific information.
    ///
    /// # Arguments
    ///
    /// * `start_address` - Starting address in the fuse memory region
    /// * `byte_count` - Number of bytes to read
    /// * `memory_id` - Memory identifier (device-specific)
    ///
    /// # Returns
    ///
    /// [`ReadMemoryResponse`] containing:
    /// - Status code of the operation
    /// - Response metadata (byte count)
    /// - Actual fuse data bytes
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if:
    /// - The operation fails (converted from status code)
    /// - Invalid response type is received
    /// - Fuse region is inaccessible or protected
    pub fn fuse_read(&mut self, start_address: u32, byte_count: u32, memory_id: u32) -> ResultComm<ReadMemoryResponse> {
        let command = CommandPacket::new_none_flag(CommandTag::FuseRead {
            start_address,
            byte_count,
            memory_id,
        });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        let status = &response.status;
        if !status.is_success() {
            return Err((*status).into());
        }
        match response.tag {
            CmdResponseTag::ReadMemory(bytes) => Ok(ReadMemoryResponse {
                status: response.status,
                response_words: Box::new([bytes.len() as u32]),
                bytes,
            }),
            _ => Err(CommunicationError::InvalidPacketReceived),
        }
    }

    /// Program fuse data
    ///
    /// Writes data to the device's fuse memory region. This operation is
    /// permanent and irreversible. Once a fuse is programmed (blown), it
    /// cannot be restored to its original state.
    ///
    /// # Arguments
    ///
    /// * `start_address` - Starting address in the fuse memory region
    /// * `memory_id` - Memory identifier (device-specific)
    /// * `bytes` - Data to write to the fuses
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Warning
    ///
    /// This operation permanently modifies the device hardware. Incorrect
    /// fuse programming can render the device unusable. Always verify the
    /// correct fuse addresses and values before programming.
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn fuse_program(&mut self, start_address: u32, memory_id: u32, bytes: &[u8]) -> ResultStatus {
        let command = CommandPacket::new_data_phase(CommandTag::FuseProgram {
            start_address,
            memory_id,
            bytes,
        });
        self.send_command(&command)?;
        let response = self.read_cmd_response()?;
        Ok(response.status)
    }

    /// Load image data directly to the device
    ///
    /// Sends raw image data to the device without a specific command header.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw image data to be loaded
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    ///
    /// # Errors
    ///
    /// Any [`CommunicationError`], almost all variants are possible.
    pub fn load_image(&mut self, bytes: &[u8]) -> ResultStatus {
        let command = CommandPacket::new_data_phase(CommandTag::NoCommand { bytes });
        self.send_command(&command)?;
        Ok(StatusCode::Success)
    }

    /// Read command response and validate status
    ///
    /// Internal helper method that reads a command response from the device
    /// and validates its status. If the status indicates an error, it converts
    /// the status code into a [`CommunicationError`].
    ///
    /// # Returns
    ///
    /// The command response if status indicates success
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] converted from the status code if the
    /// operation was not successful
    fn read_cmd_response(&mut self) -> ResultComm<CmdResponse> {
        let response = self.read_command()?;
        info!("{}: {response:02X?}", cstr!("<bold>Received"));
        if response.status.is_success() {
            Ok(response)
        } else {
            Err(response.status.into())
        }
    }
    /// Send a command packet to the device
    ///
    /// Internal helper method that handles the complete command transmission
    /// process, including data phase handling for commands that require it.
    /// For commands with data phases, it automatically queries the device's
    /// maximum packet size and splits the data accordingly.
    ///
    /// # Arguments
    ///
    /// * `command` - The command packet to send
    ///
    /// # Returns
    ///
    /// Ok(()) if the command was sent successfully
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if:
    /// - Failed to get max packet size property
    /// - Device communication fails
    /// - Data phase transmission fails
    ///
    /// # Workflow
    ///
    /// 1. Extracts parameters and data phase from the command
    /// 2. Constructs and sends the initial command packet
    /// 3. If data phase exists:
    ///    - Queries max packet size from device
    ///    - Reads intermediate response
    ///    - Splits data into chunks
    ///    - Sends each chunk with optional progress tracking
    fn send_command(&mut self, command: &CommandPacket) -> ResultComm<()> {
        let tag = &command.tag;
        let (params, data_phase) = tag.to_params();
        let packet = command.header.construct_frame(&params, tag.code());
        info!("{}: {command:02X?}", cstr!("<bold>Sending"));

        if let Some(data) = data_phase {
            info!("Sending data phase: {data:02X?}");
            let max_packet_size: u32 = {
                let response = self.get_property(PropertyTagDiscriminants::MaxPacketSize, 0)?;
                match response.property {
                    PropertyTag::MaxPacketSize(size) => size,
                    _ => return Err(CommunicationError::InvalidData),
                }
            };
            if !matches!(tag, CommandTag::NoCommand { .. }) {
                self.device.write_packet_raw(&packet)?;
                // this is the intermediate generic response
                self.read_cmd_response()?;
            }
            // Block for progress bar
            {
                let progress_bar = self.create_progress_bar(data.len() as u64, "Sending data");
                for bytes in data.chunks(
                    max_packet_size
                        .try_into()
                        .expect("pointer size of this platform is too small"),
                ) {
                    self.device.write_packet_concrete(DataPhasePacket::parse(bytes)?)?;
                    if let Some(bar) = progress_bar.as_ref() {
                        bar.inc(max_packet_size.into());
                    }
                }
            }
        } else {
            self.device.write_packet_raw(&packet)?;
        }
        Ok(())
    }

    /// Read a command response from the device
    ///
    /// Internal helper method that reads and parses command responses,
    /// handling both simple responses and those with data phases.
    ///
    /// # Returns
    ///
    /// Parsed command response
    ///
    /// # Errors
    ///
    /// Returns [`CommunicationError`] if:
    /// - Communication timeout occurs
    /// - Invalid data format is received
    /// - Command flag is unrecognized
    /// - Data phase read fails
    ///
    /// # Data Phase Handling
    ///
    /// When a response includes a data phase:
    /// 1. Reads the initial response header
    /// 2. Extracts the data phase length
    /// 3. Reads data packets until complete
    /// 4. Shows progress bar if enabled
    /// 5. Reads final status response
    fn read_command(&mut self) -> ResultComm<CmdResponse> {
        trace!("Starting to read command");
        let data = self.device.read_packet_raw(CmdResponse::get_code())?;
        let params_slice = &data[8..];

        // data[3] = param count
        if params_slice.len() % 4 != 0 && params_slice.len() != 4 * data[3] as usize {
            return Err(CommunicationError::InvalidData);
        }

        let header = CommandHeader {
            flag: CommandFlag::try_from(data[1]).or(Err(CommunicationError::InvalidData))?,
            reserved: data[2],
        };
        let status = parse_status(data[4..8].try_into().or_invalid()?)?;

        // If we dont expect data phase, we can force to return response without data
        // This is necessary for key-provisionning commands since their intermediate
        // generic response commands have data phase flag set, which is incorrect
        if self.mask_read_data_phase {
            return Ok(CmdResponse {
                header,
                status,
                tag: CmdResponseTag::from_code(data[0], params_slice, None).ok_or(CommunicationError::InvalidData)?,
            });
        }

        match header.flag {
            CommandFlag::NoData => Ok(CmdResponse {
                header,
                status,
                tag: CmdResponseTag::from_code(data[0], params_slice, None).ok_or(CommunicationError::InvalidData)?,
            }),
            CommandFlag::HasDataPhase => {
                let length = u32::from_le_bytes(params_slice[0..4].try_into().or_invalid()?);
                trace!("Data phase length: {length}");

                let mut data_phase = Vec::new();
                // Block for progress bar
                {
                    let progress_bar = self.create_progress_bar(length.into(), "Receiving data");
                    while data_phase.len() != length as usize {
                        trace!("Reading data phase packet");
                        data_phase.extend(match self.device.read_packet_concrete::<DataPhasePacket>() {
                            Ok(data) => {
                                if let Some(bar) = progress_bar.as_ref() {
                                    bar.inc(data.data.len() as u64);
                                }
                                data.data
                            }
                            Err(CommunicationError::Aborted) => break,
                            Err(err) => return Err(err),
                        });
                    }
                }

                trace!("Reading final response");
                let final_response = self.device.read_packet_raw(CmdResponse::get_code())?;
                let status = parse_status(final_response[4..8].try_into().or_invalid()?)?;

                Ok(CmdResponse {
                    header: CommandHeader {
                        flag: CommandFlag::NoData,
                        reserved: data[2],
                    },
                    status,
                    tag: CmdResponseTag::from_code(data[0], params_slice, Some(&data_phase))
                        .ok_or(CommunicationError::InvalidData)?,
                })
            }
        }
    }

    /// Create a progress bar for data transfers
    ///
    /// Internal helper method that creates a progress bar if progress tracking is enabled.
    /// The progress bar displays the transfer status with binary size formatting.
    ///
    /// # Arguments
    ///
    /// * `len` - Total length of data to transfer in bytes
    /// * `prefix` - Descriptive prefix for the progress bar
    ///
    /// # Returns
    ///
    /// Optional progress bar instance:
    /// - Some(ProgressBar) if progress tracking is enabled
    /// - None if progress tracking is disabled
    ///
    /// # Progress Bar Format
    ///
    /// The progress bar displays:
    /// - Custom prefix text
    /// - Visual progress indicator (40 characters wide)
    /// - Current bytes transferred / total bytes
    fn create_progress_bar(&self, len: u64, prefix: &'static str) -> Option<ProgressBar> {
        if self.progress_bar {
            let bar = ProgressBar::new(len);
            bar.set_style(
                ProgressStyle::with_template("{prefix} [{bar:40}] {binary_bytes:>}/{binary_total_bytes}")
                    .unwrap()
                    .progress_chars("##-"),
            );
            bar.set_prefix(prefix);
            Some(bar)
        } else {
            None
        }
    }
}

/// Parse status code from raw bytes
///
/// Converts a 4-byte little-endian value into a [`StatusCode`] enum.
/// This function is used throughout the module to interpret device responses.
///
/// # Arguments
///
/// * `data` - 4 bytes containing the status code in little-endian format
///
/// # Returns
///
/// Parsed status code enum variant
///
/// # Errors
///
/// Returns [`CommunicationError::UnexpectedStatus`] if:
/// - The status code value is not recognized
/// - The discriminant doesn't match any known [`StatusCode`] variant
///
/// The error includes both the unknown status placeholder and the actual
/// numeric value for debugging purposes.
fn parse_status(data: [u8; 4]) -> ResultComm<StatusCode> {
    let discriminant = u32::from_le_bytes(data);
    StatusCode::try_from(discriminant).or(Err(CommunicationError::UnexpectedStatus(
        StatusCode::UnknownStatusCode,
        discriminant,
    )))
}

#[cfg(test)]
mod tests {
    use crate::mboot::{
        McuBoot,
        protocols::{ProtocolOpen, uart::UARTProtocol},
        tags::property::{PropertyTag, PropertyTagDiscriminants},
    };

    const DEVICE: &str = "COM3";
    fn get_boot() -> McuBoot<UARTProtocol> {
        McuBoot::new(UARTProtocol::open(DEVICE).unwrap())
    }

    #[test]
    #[ignore = "Requires hardware connection to board"]
    fn test_board_get_version() {
        let mut boot = get_boot();
        let version = boot.get_property(PropertyTagDiscriminants::CurrentVersion, 0).unwrap();
        if let PropertyTag::CurrentVersion(ver) = version.property {
            assert_eq!(ver.mark, 'K');
            assert_eq!(ver.major, 3);
            assert_eq!(ver.minor, 1);
            assert_eq!(ver.fixation, 1);
        } else {
            panic!()
        }
    }
}
