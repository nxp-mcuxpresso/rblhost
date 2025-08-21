// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! MCU Bootloader command tag definitions and operations.
//!
//! This module contains the definitions for bootloader commands that can be sent to the device
//! to perform various operations including memory management, flash operations, device control,
//! security operations, and protocol configuration. Commands are identified by numeric tags
//! and contain parameters specific to each operation type.

use std::str::FromStr;

#[cfg(feature = "python")]
use pyo3::pyclass;

#[cfg(feature = "python")]
#[allow(clippy::wildcard_imports, reason = "Stub generation requires it like this")]
use pyo3_stub_gen::derive::*;

use crate::parsers;

use super::{ToAddress, property::PropertyTagDiscriminants};
/// MCU Bootloader Command Tags
///
/// # Command Categories
///
/// - **Memory Operations**: Read, write, fill, and erase memory regions
/// - **Flash Operations**: Specialized flash programming and security operations  
/// - **Device Control**: Reset, execute, and property management
/// - **Security**: Key provisioning, trust provisioning, and lifecycle management
/// - **Protocol Configuration**: Setup for various communication interfaces
#[repr(u8)]
#[derive(Clone, Debug, derive_more::Display, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::EnumIter))]
pub enum CommandTag<'a> {
    /// Used to load flashloader into the memory
    #[display("No Command")]
    NoCommand {
        /// Binary data to write into the device
        bytes: &'a [u8],
    } = 0x00,
    /// Erase all flash memory sectors
    #[display("Erase Complete Flash")]
    FlashEraseAll {
        /// Memory identifier (0 for internal flash)
        memory_id: u32,
    } = 0x01,

    /// Erase specific flash memory region
    #[display("Erase Flash Region")]
    FlashEraseRegion {
        /// Starting address of region to erase
        start_address: u32,
        /// Number of bytes to erase (must be sector-aligned)
        byte_count: u32,
        /// Memory identifier
        memory_id: u32,
    } = 0x02,

    /// Read data from memory
    #[display("Read Memory")]
    ReadMemory {
        /// Starting address to read from
        start_address: u32,
        /// Number of bytes to read
        byte_count: u32,
        /// Memory identifier
        memory_id: u32,
    } = 0x03,

    /// Write data to memory
    #[display("Write Memory")]
    WriteMemory {
        /// Starting address to write to
        start_address: u32,
        /// Memory identifier
        memory_id: u32,
        /// Binary data to write
        bytes: &'a [u8],
    } = 0x04,

    /// Fill memory region with pattern
    #[display("Fill Memory")]
    FillMemory {
        /// Starting address (must be word-aligned)
        start_address: u32,
        /// Number of bytes to fill (must be word-aligned)
        byte_count: u32,
        /// 32-bit pattern to repeat
        pattern: u32,
    } = 0x05,

    /// Disable flash read/write protection
    #[display("Disable Flash Security")]
    FlashSecurityDisable = 0x06,

    /// Get device property value
    #[display("Get Property")]
    GetProperty {
        /// Property identifier to retrieve
        tag: PropertyTagDiscriminants,
        /// Memory index for memory-specific properties
        memory_index: u32,
    } = 0x07,

    /// Process Secure Binary (SB) file
    #[display("Receive SB File")]
    ReceiveSBFile {
        /// SB file binary data
        bytes: &'a [u8],
    } = 0x08,

    /// Execute code at specified address
    #[display("Execute")]
    Execute {
        /// Address to jump to
        start_address: u32,
        /// Argument passed to the code
        argument: u32,
        /// Stack pointer value to set
        stackpointer: u32,
    } = 0x09,

    /// Call function
    #[display("Call")]
    Call {
        /// Function address to call
        start_address: u32,
        /// Argument passed to function
        argument: u32,
    } = 0x0A,

    /// Reset the MCU
    #[display("Reset MCU")]
    Reset = 0x0B,

    /// Set device property value
    #[display("Set Property")]
    SetProperty {
        /// Property identifier to set
        tag: PropertyTagDiscriminants,
        /// New property value
        value: u32,
    } = 0x0C,

    /// Erase all flash and remove security
    #[display("Erase Complete Flash and Unlock")]
    FlashEraseAllUnsecure = 0x0D,

    /// Program One-Time Programmable (OTP) memory
    #[display("Flash Program Once")]
    FlashProgramOnce {
        /// OTP memory index
        index: u32,
        /// Number of bytes to program
        count: u32,
        /// Data to program
        data: u32,
    } = 0x0E,

    /// Read One-Time Programmable (OTP) memory
    #[display("Flash Read Once")]
    FlashReadOnce {
        /// OTP memory index
        index: u32,
        /// Number of bytes to read
        count: u32,
    } = 0x0F,

    /// Read flash resource information
    #[display("Flash Read Resource")]
    FlashReadResource = 0x10,

    /// Configure external memory interface
    #[display("Configure Quad-SPI Memory")]
    ConfigureMemory {
        /// Memory interface identifier
        memory_id: u32,
        /// Configuration data address
        address: u32,
    } = 0x11,

    /// Perform reliable update operation
    #[display("Reliable Update")]
    ReliableUpdate = 0x12,

    /// Generate encrypted key blob
    #[display("Generate Key Blob")]
    GenerateKeyBlob = 0x13,

    /// Program device fuses
    #[display("Program Fuse")]
    FuseProgram {
        /// Starting fuse address
        start_address: u32,
        /// Fuse data to program
        bytes: &'a [u8],
        /// Memory identifier
        memory_id: u32,
    } = 0x14,

    /// Key provisioning operations
    #[display("Key Provisioning")]
    KeyProvisioning(&'a KeyProvOperation) = 0x15,

    /// Trust provisioning operations
    #[display("Trust Provisioning")]
    TrustProvisioning(&'a TrustProvOperation) = 0x16,

    /// Read device fuses
    #[display("Read Fuse")]
    FuseRead {
        /// Starting fuse address
        start_address: u32,
        /// Number of bytes to read
        byte_count: u32,
        /// Memory identifier
        memory_id: u32,
    } = 0x17,

    /// Update device lifecycle state
    #[display("Update Life Cycle")]
    UpdateLifeCycle = 0x18,

    /// Send EdgeLock Enclave message
    #[display("Send EdgeLock Enclave Message")]
    EleMessage = 0x19,

    /// EdgeLock 2GO provisioning operations
    #[display("EL2GO Provisioning Commands and API Calls")]
    EL2GO = 0x20,

    // Protocol configuration commands (reserved range)
    /// Configure I2C interface parameters
    #[display("Configure I2C")]
    ConfigureI2C = 0xC1,

    /// Configure SPI interface parameters
    #[display("Configure SPI")]
    ConfigureSPI = 0xC2,

    /// Configure CAN interface parameters
    #[display("Configure CAN")]
    ConfigureCAN = 0xC3,
}
impl CommandToParams for CommandTag<'_> {
    /// Convert command to parameters and optional data phase.
    ///
    /// Converts the command into a tuple containing command parameters as a vector of u32 values
    /// and an optional byte slice for data phase transmission.
    ///
    /// # Returns
    /// A tuple where the first element contains command parameters and the second contains
    /// optional data phase bytes
    fn to_params(&self) -> (Vec<u32>, Option<&[u8]>) {
        match *self {
            CommandTag::FlashEraseAll { memory_id } => (vec![memory_id], None),
            CommandTag::ReadMemory {
                start_address,
                byte_count,
                memory_id,
            }
            | CommandTag::FlashEraseRegion {
                start_address,
                byte_count,
                memory_id,
            }
            | CommandTag::FuseRead {
                start_address,
                byte_count,
                memory_id,
            } => (vec![start_address, byte_count, memory_id], None),
            CommandTag::WriteMemory {
                start_address,
                memory_id,
                bytes,
            }
            | CommandTag::FuseProgram {
                start_address,
                memory_id,
                bytes,
            } => (vec![start_address, bytes.len() as u32, memory_id], Some(bytes)),
            CommandTag::FillMemory {
                start_address,
                byte_count,
                pattern,
            } => (vec![start_address, byte_count, pattern], None),
            CommandTag::GetProperty { tag, memory_index } => (vec![u8::from(tag).into(), memory_index], None),
            CommandTag::Reset | CommandTag::FlashEraseAllUnsecure => (vec![], None),
            CommandTag::SetProperty { tag, value } => (vec![u8::from(tag).into(), value], None),
            CommandTag::ConfigureMemory { memory_id, address } => (vec![memory_id, address], None),
            CommandTag::ReceiveSBFile { bytes } | CommandTag::NoCommand { bytes } => {
                (vec![bytes.len() as u32], Some(bytes))
            }
            CommandTag::TrustProvisioning(operation) => operation.to_params(),
            CommandTag::KeyProvisioning(operation) => operation.to_params(),
            CommandTag::FlashReadOnce { index, count } => (vec![index, count], None),
            CommandTag::FlashProgramOnce { index, count, data } => (vec![index, count, data], None),
            CommandTag::Execute {
                start_address,
                argument,
                stackpointer,
            } => (vec![start_address, argument, stackpointer], None),
            CommandTag::Call {
                start_address,
                argument,
            } => (vec![start_address, argument], None),
            // remove this once all commands are added
            _ => unimplemented!("this command has not yet been implemented"),
        }
    }
}

impl From<CommandTagDiscriminants> for u8 {
    /// Convert command tag discriminant to its numeric representation.
    fn from(value: CommandTagDiscriminants) -> Self {
        // enum is repr(u8)
        value as u8
    }
}

impl ToAddress for CommandTag<'_> {}

/// Trait for converting commands to parameters and data phase.
pub trait CommandToParams {
    /// Convert command to parameters and optional data phase.
    ///
    /// The first item in tuple are command parameters, the second are bytes to be sent in data phase.
    #[must_use]
    fn to_params(&self) -> (Vec<u32>, Option<&[u8]>);
}
/// Trust provisioning operations for device security setup.
///
/// These operations handle OEM master share generation and configuration
/// for establishing device trust relationships.
#[cfg_attr(feature = "python", gen_stub_pyclass_enum, pyclass(eq, name = "TrustProvOperation"))]
#[derive(PartialEq, Eq, clap::Subcommand, Clone, Copy, Debug, derive_more::Display)]
#[command(rename_all = "snake_case")]
pub enum TrustProvOperation {
    /// Generate OEM master share for initial trust provisioning
    #[display("Enroll Operation")]
    OemGenMasterShare {
        /// Input buffer address containing the OEM Share (entropy seed)
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_share_input_addr: u32,

        /// Size of the OEM Share entropy seed in bytes
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_share_input_size: u32,

        /// Output buffer address for the Encrypted OEM Share
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_enc_share_output_addr: u32,

        /// Size of the encrypted OEM share output buffer in bytes
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_enc_share_output_size: u32,

        /// Output buffer address for the Encrypted OEM Master Share
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_enc_master_share_output_addr: u32,

        /// Size of the encrypted OEM master share output buffer in bytes
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_enc_master_share_output_size: u32,

        /// Output buffer address for the OEM Customer Certificate Public Key
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_cust_cert_puk_output_addr: u32,

        /// Size of the customer certificate public key output buffer in bytes
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_cust_cert_puk_output_size: u32,
    },

    /// Set OEM master share to complete trust provisioning
    #[display("Set User Key Operation")]
    OemSetMasterShare {
        /// Input buffer address containing the OEM Share (entropy seed)
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_share_input_addr: u32,

        /// Size of the OEM Share entropy seed in bytes
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_share_input_size: u32,

        /// Input buffer address containing the Encrypted OEM Master Share
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_enc_master_share_input_addr: u32,

        /// Size of the Encrypted OEM Master Share in bytes
        #[arg(value_parser=parsers::parse_number::<u32>)]
        oem_enc_master_share_input_size: u32,
    },
}
impl CommandToParams for TrustProvOperation {
    /// Convert trust provisioning operation to command parameters.
    ///
    /// # Returns
    /// Tuple containing operation parameters and no data phase
    fn to_params(&self) -> (Vec<u32>, Option<&[u8]>) {
        match *self {
            TrustProvOperation::OemGenMasterShare {
                oem_share_input_addr,
                oem_share_input_size,
                oem_enc_share_output_addr,
                oem_enc_share_output_size,
                oem_enc_master_share_output_addr,
                oem_enc_master_share_output_size,
                oem_cust_cert_puk_output_addr,
                oem_cust_cert_puk_output_size,
            } => (
                vec![
                    0,
                    oem_share_input_addr,
                    oem_share_input_size,
                    oem_enc_share_output_addr,
                    oem_enc_share_output_size,
                    oem_enc_master_share_output_addr,
                    oem_enc_master_share_output_size,
                    oem_cust_cert_puk_output_addr,
                    oem_cust_cert_puk_output_size,
                ],
                None,
            ),
            TrustProvOperation::OemSetMasterShare {
                oem_share_input_addr,
                oem_share_input_size,
                oem_enc_master_share_input_addr,
                oem_enc_master_share_input_size,
            } => (
                vec![
                    1,
                    oem_share_input_addr,
                    oem_share_input_size,
                    oem_enc_master_share_input_addr,
                    oem_enc_master_share_input_size,
                ],
                None,
            ),
        }
    }
}

/// Key provisioning operations for device key management.
///
/// These operations handle enrollment, key setting, and key store management
/// for various types of encryption and authentication keys.
#[derive(clap::Subcommand, Clone, Debug, derive_more::Display)]
#[command(rename_all = "snake_case")]
pub enum KeyProvOperation {
    #[display("Enroll Operation")]
    /// Enrolls key provisioning feature. No arguments are required for this operation
    Enroll,

    #[display("Set User Key Operation")]
    /// Sends the user key specified by type to the bootloader.
    ///
    /// Available KEY TYPES:
    ///  2 or 'OTFADKEK'    OTFAD key
    ///  3 or 'SBKEK'       SB file encryption key
    ///  7 or 'PRINCE0'     Prince region 0 encryption key
    ///  8 or 'PRINCE1'     Prince region 1 encryption key
    ///  9 or 'PRINCE2'     Prince region 2 encryption key
    /// 11 or 'USERKEK'     User/Boot-image encryption key
    /// 12 or 'UDS'         Universal Device Secret for DICE
    #[command(verbatim_doc_comment)]
    SetUserKey {
        /// Type of user key
        #[arg(value_parser=KeyProvUserKeyType::parse, id = "key", verbatim_doc_comment)]
        key_type: KeyProvUserKeyType,

        /// Binary file containing user key plaintext
        #[arg(value_parser = |s: &str| parsers::parse_file(s, None))]
        key_data: Box<[u8]>,
    },

    #[display("Set Key Operation")]
    /// Generates a key of specified size and type on the device.
    ///
    /// Available KEY TYPES:
    ///  2 or 'OTFADKEK'    OTFAD key
    ///  3 or 'SBKEK'       SB file encryption key
    ///  7 or 'PRINCE0'     Prince region 0 encryption key
    ///  8 or 'PRINCE1'     Prince region 1 encryption key
    ///  9 or 'PRINCE2'     Prince region 2 encryption key
    /// 11 or 'USERKEK'     User/Boot-image encryption key
    /// 12 or 'UDS'         Universal Device Secret for DICE
    ///
    /// Note: The valid options of key type and corresponding size are documented
    /// in the target's Reference Manual or User Manual.
    /// Note: Names are case insensitive
    #[command(verbatim_doc_comment)]
    SetKey {
        /// Type of key to generate
        #[arg(value_parser=KeyProvUserKeyType::parse)]
        key_type: KeyProvUserKeyType,

        /// Size of key to generate in bytes
        #[arg(value_parser=parsers::parse_number::<u32>)]
        key_size: u32,
    },

    #[display("Write Key Nonvolatile Operation")]
    /// Writes data to non-volatile storage
    WriteKeyNonvolatile {
        /// ID of the non-volatile memory
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t = 0)]
        memory_id: u32,
    },

    #[display("Read Key Nonvolatile Operation")]
    /// Loads the key from nonvolatile memory to bootloader.
    ReadKeyNonvolatile {
        /// ID of the non-volatile memory
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t = 0)]
        memory_id: u32,
    },
    /// Write key store data to the bootloader
    #[display("Write Key Store Operation")]
    WriteKeyStore {
        /// Binary file containing key store data
        #[arg(value_parser = |s: &str| parsers::parse_file(s, None))]
        keystore_data: Box<[u8]>,
    },

    #[display("Read Key Store Operation")]
    /// Reads the key store from the bootloader to host
    ReadKeyStore {
        /// Binary file to save the key store
        file: String,

        /// Use hexdump format
        #[arg(long, short, default_value_t = false)]
        use_hexdump: bool,
    },
}

impl CommandToParams for KeyProvOperation {
    /// Convert key provisioning operation to command parameters.
    ///
    /// # Returns
    /// Tuple containing operation parameters and optional key data for data phase
    fn to_params(&self) -> (Vec<u32>, Option<&[u8]>) {
        match *self {
            KeyProvOperation::Enroll => (vec![0], None),
            KeyProvOperation::SetUserKey { key_type, ref key_data } => {
                // For SetUserKey, we need to include the key type and length in the parameters
                // and the actual key data in the data phase
                (vec![1, key_type.into(), key_data.len() as u32], Some(key_data))
            }
            KeyProvOperation::SetKey { key_type, key_size } => {
                // For SetKey (intrinsic key generation), we pass the key type and size
                // This corresponds to the Python mboot.kp_set_intrinsic_key(key_type_int, key_size)
                (vec![2, key_type.into(), key_size], None)
            }
            KeyProvOperation::WriteKeyNonvolatile { memory_id } => (vec![3, memory_id], None),
            KeyProvOperation::ReadKeyNonvolatile { memory_id } => (vec![4, memory_id], None),
            KeyProvOperation::WriteKeyStore { ref keystore_data } => {
                (vec![5, 0, keystore_data.len() as u32], Some(keystore_data))
            }
            KeyProvOperation::ReadKeyStore { .. } => (vec![6], None),
        }
    }
}

/// User key types for key provisioning operations.
///
/// Defines the different types of keys that can be provisioned on the device
/// for various encryption and security functions.
#[cfg_attr(
    feature = "python",
    gen_stub_pyclass_enum,
    pyclass(eq, eq_int, name = "KeyProvUserKeyType")
)]
#[repr(u32)]
#[derive(PartialEq, Eq, derive_more::TryFrom, derive_more::Display, Clone, Copy, Debug, strum::EnumString)]
#[try_from(repr)]
#[strum(serialize_all = "UPPERCASE")]
pub enum KeyProvUserKeyType {
    /// OTFAD (On-The-Fly AES Decryption) encryption key
    #[strum(serialize = "OTFADKEK")]
    #[display("Key for OTFAD encryption")]
    OtfadKek = 2,
    /// Secure Boot file encryption key
    #[strum(serialize = "SBKEK")]
    #[display("Key for SB file encryption")]
    SbKek = 3,
    /// Prince region 0 encryption key
    #[strum(serialize = "PRINCE0")]
    #[display("Key for Prince region 0")]
    PrinceRegion0 = 7,
    /// Prince region 1 encryption key
    #[strum(serialize = "PRINCE1")]
    #[display("Key for Prince region 1")]
    PrinceRegion1 = 8,
    /// Prince region 2 encryption key
    #[strum(serialize = "PRINCE2")]
    #[display("Key for Prince region 2")]
    PrinceRegion2 = 9,
    /// Prince region 3 encryption key
    #[strum(serialize = "PRINCE3")]
    #[display("Key for Prince region 3")]
    PrinceRegion3 = 10,
    /// User/Boot image encryption key
    #[strum(serialize = "USERKEK")]
    #[display("Encrypted boot image key")]
    UserKek = 11,
    /// Universal Device Secret for DICE (Device Identifier Composition Engine)
    #[strum(serialize = "UDS")]
    #[display("Universal Device Secret for DICE")]
    Uds = 12,
}
impl From<KeyProvUserKeyType> for u32 {
    /// Convert key provisioning user key type to its numeric representation.
    fn from(value: KeyProvUserKeyType) -> Self {
        value as u32
    }
}

impl KeyProvUserKeyType {
    /// Parse key provisioning user key type from string input.
    ///
    /// Attempts to parse the input as either a numeric value or a string name.
    ///
    /// # Arguments
    /// * `s` - String input to parse
    ///
    /// # Returns
    /// Result containing the parsed [`KeyProvUserKeyType`] or an error message
    ///
    /// # Errors
    /// Text containing the error, either invalid ID or invalid name for [`KeyProvUserKeyType`].
    pub fn parse(s: &str) -> Result<KeyProvUserKeyType, String> {
        if s.chars().all(char::is_numeric) {
            let res = parsers::parse_number::<u32>(s)?;
            KeyProvUserKeyType::try_from(res).or(Err("invalid ID".to_owned()))
        } else {
            KeyProvUserKeyType::from_str(s).or(Err("invalid name".to_owned()))
        }
    }
}
