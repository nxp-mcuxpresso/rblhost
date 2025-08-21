// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! Bootloader property tag definitions and parsing.
//!
//! This module contains the definitions and parsing logic for bootloader properties
//! that can be queried to get information about device capabilities, memory layout,
//! and current status. Properties are identified by numeric tags and contain
//! various types of data including version information, memory addresses and sizes,
//! peripheral availability, and device configuration.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

#[cfg(feature = "python")]
use pyo3::pyclass;

#[cfg(feature = "python")]
#[allow(clippy::wildcard_imports, reason = "Stub generation requires it like this")]
use pyo3_stub_gen::derive::*;

use strum::IntoEnumIterator;

use crate::{
    mboot::{
        formatters::{BinaryBytesOne, OnOffBool},
        memory::{ExternalMemoryAttributes, ReservedRegions},
    },
    parsers::parse_number,
};

use super::{ToAddress, command::CommandTagDiscriminants, status::StatusCode};
/// Wrapper type for device identification bytes.
///
/// Contains the device identification number as a sequence of bytes.
#[derive(Clone, Debug)]
pub struct DeviceId(Box<[u8]>);
impl Display for DeviceId {
    /// Format device ID as hexadecimal string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.0.iter().fold(String::new(), |acc, i| acc + &format!("{i:02x}"));
        write!(f, "{bytes}")?;
        Ok(())
    }
}

/// Enumeration of bootloader property tags.
///
/// These properties can be queried from the bootloader to get information about
/// the device capabilities, memory layout, and current status.
#[repr(u8)]
#[derive(Clone, Debug, strum::EnumDiscriminants, derive_more::Display)]
#[strum_discriminants(
    derive(derive_more::TryFrom, strum::EnumString),
    try_from(repr),
    strum(serialize_all = "kebab-case"),
    cfg_attr(feature = "python", gen_stub_pyclass_enum, pyclass(eq, eq_int, name = "PropertyTag"))
)]
pub enum PropertyTag {
    /// Current version of the bootloader
    #[display("Current Version = {_0}")]
    CurrentVersion(Version) = 0x01,
    /// List of available peripheral interfaces
    #[display("Available Peripherals = {}", _0
        .iter()
        .map(|per| format!("{per:?}"))
        .reduce(|acc, i| acc + ", " + &i).
        unwrap_or_default()
    )]
    AvailablePeripherals(Box<[PeripheryTag]>) = 0x02,
    /// Start address of the internal flash memory
    ///
    /// # Note
    /// The length includes the "0x" part.
    #[display("Flash Start Address = {_0:#010X}")]
    FlashStartAddress(u32) = 0x03,
    /// Size of the internal flash memory
    #[display("Flash Size = {}", BinaryBytesOne(*_0))]
    FlashSize(u32) = 0x04,
    /// Size of a single flash sector
    #[display("Flash Sector Size = {}", BinaryBytesOne(*_0))]
    FlashSectorSize(u32) = 0x05,
    /// Number of flash blocks
    #[display("Flash Block Count = {_0}")]
    FlashBlockCount(u32) = 0x06,
    /// List of available bootloader commands
    #[display("Available Commands = [{}]", _0
        .iter()
        .map(|cmd| format!("'{cmd:?}'"))
        .reduce(|acc, i| acc + ", " + &i)
        .unwrap_or_default()
    )]
    AvailableCommands(Box<[CommandTagDiscriminants]>) = 0x07,
    /// Status of the last CRC check operation
    #[display("CRC Check Status = {_0:?}")]
    CRCCheckStatus(StatusCode) = 0x08,
    /// Value of the last error that occurred
    #[display("Last Error Value = {_0}")]
    LastError(u32) = 0x09,
    /// Whether write operations are verified after completion
    #[display("Verify Writes = {}", OnOffBool(*_0))]
    VerifyWrites(bool) = 0x0A,
    /// Maximum packet size for data transfer
    #[display("Max Packet Size = {}", BinaryBytesOne(*_0))]
    MaxPacketSize(u32) = 0x0B,
    /// Memory regions reserved by the bootloader
    #[display("Reserved Regions =\n{_0}")]
    ReservedRegions(ReservedRegions) = 0x0C,
    /// Regions that should be validated
    #[display("Validate Regions = {_0}")]
    ValidateRegions(bool) = 0x0D,
    /// Start address of the internal RAM memory
    #[display("RAM Start Address = {_0:#010X}")]
    RAMStartAddress(u32) = 0x0E,
    /// Size of the internal RAM memory
    #[display("RAM Size = {}", BinaryBytesOne(*_0))]
    RAMSize(u32) = 0x0F,
    /// System device identification number
    #[display("System Device Identification = {_0:#010X}")]
    SystemDeviceId(u32) = 0x10,
    /// Current security state of the flash memory
    #[display("Security State = {_0}")]
    FlashSecurityState(FlashSecurityState) = 0x11,
    /// Unique device identification bytes
    #[display("Unique Device Identification = {_0}")]
    UniqueDeviceId(DeviceId) = 0x12,
    /// Flash factory support information
    #[display("Flash Fac. Support = {_0}")]
    FlashFacSupport(bool) = 0x13,
    /// Size of flash access segments
    #[display("Flash Access Segment Size = {}", BinaryBytesOne(*_0))]
    FlashAccessSegmentSize(u32) = 0x14,
    /// Number of flash access segments
    #[display("Flash Access Segment Count = {_0}")]
    FlashAccessSegmentCount(u32) = 0x15,
    /// Flash read margin setting
    #[display("Flash Read Margin = {_0}")]
    FlashReadMargin(FlashReadMargin) = 0x16,
    /// QuadSPI initialization status
    #[display("QuadSPI Initialization Status = {_0}")]
    QSPIInitStatus(StatusCode) = 0x17,
    /// Target version information
    #[display("Target Version = {_0}")]
    TargetVersion(Version) = 0x18,
    // FIXME Was not properly tested
    /// Attributes of external memory devices
    #[display("External Memory Attributes = {_0}")]
    ExternalMemoryAttributes(ExternalMemoryAttributes) = 0x19,
    /// Status of reliable update feature
    #[display("Reliable Update Status = {_0}")]
    ReliableUpdateStatus(StatusCode) = 0x1A,
    /// Size of a single flash page
    #[display("Flash Page Size = {}", BinaryBytesOne(*_0))]
    FlashPageSize(u32) = 0x1B,
    /// IRQ notifier pin configuration
    #[display("Irq Notifier Pin = {_0}")]
    IrqNotifierPin(IrqNotifierPin) = 0x1C,
    /// PFR keystore update option
    #[display("PFR Keystore Update Opt = {_0}")]
    PFRKeystoreUpdateOpt(PfrKeystoreUpdateOpt) = 0x1D,
    /// Timeout for byte write operations in milliseconds
    #[display("Byte Write Timeout in ms = {_0}")]
    ByteWriteTimeoutMs(u32) = 0x1E,
    /// Status of fuse locked state
    #[display("Fuse Locked Status")]
    FuseLockedStatus = 0x1F,
    /// Boot status register value
    #[display("Boot Status Register = {_0}")]
    BootStatusRegister(u32) = 0x20,
    /// Firmware version information
    #[display("Firmware Version = {_0}")]
    FirmwareVersion(u32) = 0x21,
    /// Fuse program voltage setting
    #[display("Fuse Program Voltage = {_0}")]
    FuseProgramVoltage(FuseProgramVoltage) = 0x22,
    /// Whether erase operations are verified after completion
    #[display("Verify Erase = {_0}")]
    VerifyErase(bool) = 0x23,
    /// Secure Hardware Extension flash partition information
    #[display("SHE Flash Partition = {_0}")]
    SHEFlashPartition(SHEFlashPartition) = 0x24,
    /// Secure Hardware Extension boot mode information
    #[display("SHE Boot Mode = {_0}")]
    SHEBootMode(SHEBootMode) = 0x25,
    /// Current life cycle state of the device
    #[display("Life Cycle State = {_0}")]
    LifeCycleState(LifeCycleState) = 0x26,
}

type PTag = PropertyTag;
type PTagDisc = PropertyTagDiscriminants;
impl PTag {
    /// Create a [`PropertyTag`] from a discriminant and data array.
    ///
    /// Parses the raw data according to the property type and creates
    /// the appropriate [`PropertyTag`] variant.
    ///
    /// # Arguments
    /// * `tag` - Property tag discriminant identifying the property type
    /// * `data` - Raw data array containing the property value
    ///
    /// # Returns
    /// Parsed [`PropertyTag`] variant
    ///
    /// # Panics
    /// When parsing [`PropertyTag::CRCCheckStatus`], if the status returned by the board is invalid.
    #[must_use]
    pub fn from_code(tag: PTagDisc, data: &[u32]) -> PTag {
        match tag {
            PTagDisc::CurrentVersion => PTag::CurrentVersion(Version::parse(data[0])),
            PTagDisc::TargetVersion => PTag::TargetVersion(Version::parse(data[0])),
            PTagDisc::UniqueDeviceId => {
                let bytes = data.iter().flat_map(|val| val.to_le_bytes()).collect();
                PTag::UniqueDeviceId(DeviceId(bytes))
            }
            PTagDisc::AvailablePeripherals => {
                // truncating all unnecessary bits
                let num = data[0] as u8;
                let v = PeripheryTag::iter().filter(|per| u8::from(*per) & num != 0).collect();
                PTag::AvailablePeripherals(v)
            }
            PTagDisc::FlashStartAddress => PTag::FlashStartAddress(data[0]),
            PTagDisc::FlashSize => PTag::FlashSize(data[0]),
            PTagDisc::FlashSectorSize => PTag::FlashSectorSize(data[0]),
            PTagDisc::AvailableCommands => PTag::AvailableCommands(
                CommandTagDiscriminants::iter()
                    .filter(|tag| {
                        let tag_value = u8::from(*tag);
                        (0 < tag_value && tag_value < 0xA0) && {
                            let mask = 1 << (tag_value - 1);
                            data[0] & mask != 0
                        }
                    })
                    .collect(),
            ),
            PTagDisc::CRCCheckStatus => {
                PTag::CRCCheckStatus(StatusCode::try_from(data[0]).expect("board returned invalid CRC status"))
            }
            PTagDisc::VerifyWrites => PTag::VerifyWrites(data[0] != 0),
            PTagDisc::MaxPacketSize => PTag::MaxPacketSize(data[0]),
            PTagDisc::ReservedRegions => PTag::ReservedRegions(ReservedRegions::parse(&data[2..])),
            PTagDisc::RAMStartAddress => PTag::RAMStartAddress(data[0]),
            PTagDisc::RAMSize => PTag::RAMSize(data[0]),
            PTagDisc::SystemDeviceId => PTag::SystemDeviceId(data[0]),
            PTagDisc::FlashSecurityState => {
                PTag::FlashSecurityState(FlashSecurityState(data[0] == 0x0 || data[0] == 0x5AA55AA5))
            }
            PTagDisc::ExternalMemoryAttributes => PTag::ExternalMemoryAttributes(ExternalMemoryAttributes::parse(data)),
            PTagDisc::FlashPageSize => PTag::FlashPageSize(data[0]),
            PTagDisc::IrqNotifierPin => PTag::IrqNotifierPin(IrqNotifierPin::parse(data[0])),
            PTagDisc::PFRKeystoreUpdateOpt => PTag::PFRKeystoreUpdateOpt(PfrKeystoreUpdateOpt::parse(data[0])),
            PTagDisc::ByteWriteTimeoutMs => PTag::ByteWriteTimeoutMs(data[0]),
            PTagDisc::BootStatusRegister => PTag::BootStatusRegister(data[0]),
            PTagDisc::FirmwareVersion => PTag::FirmwareVersion(data[0]),
            PTagDisc::FuseProgramVoltage => PTag::FuseProgramVoltage(FuseProgramVoltage::parse(data[0])),
            PTagDisc::VerifyErase => PTag::VerifyErase(data[0] != 0),
            PTagDisc::SHEFlashPartition => PTag::SHEFlashPartition(SHEFlashPartition::parse(data[0])),
            PTagDisc::SHEBootMode => PTag::SHEBootMode(SHEBootMode::parse(data[0])),
            PTagDisc::LifeCycleState => PTag::LifeCycleState(LifeCycleState(data[0] == 0x0 || data[0] == 0x5AA55AA5)),
            PTagDisc::FlashBlockCount => PTag::FlashBlockCount(data[0]),
            PTagDisc::FlashAccessSegmentCount => PTag::FlashAccessSegmentCount(data[0]),
            PTagDisc::ValidateRegions => PTag::ValidateRegions(data[0] != 0),
            PTagDisc::FlashFacSupport => PTag::FlashFacSupport(data[0] != 0),
            PTagDisc::FlashAccessSegmentSize => PTag::FlashAccessSegmentSize(data[0]),
            PTagDisc::FlashReadMargin => PTag::FlashReadMargin(FlashReadMargin::parse(data[0])),
            PTagDisc::QSPIInitStatus => {
                PTag::QSPIInitStatus(StatusCode::try_from(data[0]).expect("board returned invalid QSPI init status"))
            }
            PTagDisc::ReliableUpdateStatus => PTag::ReliableUpdateStatus(
                StatusCode::try_from(data[0]).expect("board returned invalid Reliable update status"),
            ),
            // TODO: Implement parsing for any remaining property tag discriminants
            PTagDisc::FuseLockedStatus => unimplemented!("Fuse Locked Status parsing not yet implemented"),
            PTagDisc::LastError => unimplemented!("Last Error parsing not yet implemented"),
        }
    }
}
impl PTagDisc {
    /// Parse property tag discriminant from string input.
    ///
    /// Attempts to parse the input as either a numeric value or a string name.
    ///
    /// # Arguments
    /// * `s` - String input to parse
    ///
    /// # Returns
    /// Result containing the parsed [`PropertyTagDiscriminants`] or an error message
    ///
    /// # Errors
    /// Text containing the error, either invalid number or invalid name for [`PropertyTagDiscriminants`].
    pub fn parse_property(s: &str) -> Result<PropertyTagDiscriminants, &'static str> {
        match parse_number::<u8>(s) {
            Ok(num) => PropertyTagDiscriminants::try_from(num).or(Err("Could not find property with this number")),
            Err(_) => PropertyTagDiscriminants::from_str(s).or(Err("Property with this name does not exist")),
        }
    }
}

impl From<PTagDisc> for u8 {
    /// Convert property tag discriminant to its numeric representation.
    fn from(value: PTagDisc) -> Self {
        value as u8
    }
}

impl ToAddress for PTag {}

/// Version information structure.
///
/// Contains version components including a character mark and numeric version parts.
#[derive(Clone, Copy, Debug)]
pub struct Version {
    /// Version mark character
    pub mark: char,
    /// Major version number
    pub major: u8,
    /// Minor version number
    pub minor: u8,
    /// Fixation version number
    pub fixation: u8,
}

impl Version {
    /// Parse version from a 32-bit integer.
    ///
    /// Extracts version components from big-endian byte representation.
    ///
    /// # Arguments
    /// * `num` - 32-bit integer containing packed version information
    ///
    /// # Returns
    /// Version structure with parsed components
    ///
    /// # Panics
    /// Panics if the first item in `num` is not a valid character.
    #[must_use]
    pub fn parse(num: u32) -> Self {
        let bytes = num.to_be_bytes();
        Version {
            mark: char::from_u32(bytes[0].into()).unwrap(),
            major: bytes[1],
            minor: bytes[2],
            fixation: bytes[3],
        }
    }
}

impl Display for Version {
    /// Format version as string in the format "M#.#.#" where M is the mark character.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}.{}.{}", self.mark, self.major, self.minor, self.fixation)
    }
}

/// Enumeration of peripheral interface types supported by the bootloader.
///
/// These represent the different communication interfaces that can be used
/// to communicate with the bootloader.
#[repr(u8)]
#[derive(Clone, Copy, strum::EnumIter, derive_more::Debug, derive_more::Display)]
pub enum PeripheryTag {
    /// UART serial interface
    #[display("UART Interface")]
    #[debug("UART")]
    Uart = 0x01,
    /// I2C slave interface
    #[display("I2C Slave Interface")]
    #[debug("I2C-Slave")]
    I2CSlave = 0x02,
    /// SPI slave interface
    #[display("SPI Slave Interface")]
    #[debug("SPI-Slave")]
    SPISlave = 0x04,
    /// CAN bus interface
    #[display("CAN Interface")]
    #[debug("CAN")]
    Can = 0x08,
    /// USB Human Interface Device (HID) class interface
    #[display("USB HID-Class Interface")]
    #[debug("USB-HID")]
    UsbHid = 0x10,
    /// USB Communication Device Class (CDC) interface
    #[display("USB CDC-Class Interface")]
    #[debug("USB-CDC")]
    UsbCdc = 0x20,
    /// USB Device Firmware Upgrade (DFU) class interface
    #[display("USB DFU-Class Interface")]
    #[debug("USB-DFU")]
    UsbDfu = 0x40,
    /// Local Interconnect Network (LIN) interface
    #[display("LIN Interface")]
    #[debug("LIN")]
    Lin = 0x80,
}

impl From<PeripheryTag> for u8 {
    /// Convert periphery tag to its numeric representation.
    fn from(value: PeripheryTag) -> Self {
        value as u8
    }
}

/// Flash security state information.
///
/// Indicates whether the flash memory is in secure or unsecure state.
#[derive(Clone, Copy, Debug)]
pub struct FlashSecurityState(pub bool);

// TODO implement board overrides for properties
impl Display for FlashSecurityState {
    /// Format flash security state as "SECURE" or "UNSECURE".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.0 { "UNSECURE" } else { "SECURE" };
        write!(f, "{state}")
    }
}

/// IRQ notifier pin configuration.
///
/// Contains information about the IRQ notifier pin including port, pin number, and enabled state.
#[derive(Clone, Copy, Debug)]
pub struct IrqNotifierPin {
    /// Pin number
    pin: u8,
    /// Port number
    port: u8,
    /// Whether the IRQ notifier is enabled
    enabled: bool,
}

impl IrqNotifierPin {
    /// Parse IRQ notifier pin configuration from a 32-bit value.
    ///
    /// # Arguments
    /// * `value` - 32-bit value containing packed pin configuration
    ///
    /// # Returns
    /// Parsed IRQ notifier pin configuration
    #[must_use]
    pub fn parse(value: u32) -> Self {
        IrqNotifierPin {
            // ANDing here always ensures, that the value is in u8 range
            pin: (value & 0xFF) as u8,
            port: ((value >> 8) & 0xFF) as u8,
            enabled: (value & (1 << 31)) > 0,
        }
    }
}

impl Display for IrqNotifierPin {
    /// Format IRQ notifier pin configuration showing port, pin, and enabled status.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.enabled { "enabled" } else { "disabled" };
        write!(f, "IRQ Port[{}], Pin[{}] is {status}", self.port, self.pin)
    }
}

/// PFR keystore update option enumeration.
///
/// Specifies the method used for PFR keystore updates.
#[derive(Clone, Copy, Debug)]
pub enum PfrKeystoreUpdateOpt {
    /// Key provisioning method
    KeyProvisioning = 0,
    /// Write memory method  
    WriteMemory = 1,
}

impl Display for PfrKeystoreUpdateOpt {
    /// Format PFR keystore update option as string representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PfrKeystoreUpdateOpt::KeyProvisioning => write!(f, "KEY_PROVISIONING"),
            PfrKeystoreUpdateOpt::WriteMemory => write!(f, "WRITE_MEMORY"),
        }
    }
}

impl PfrKeystoreUpdateOpt {
    #[must_use]
    pub fn parse(value: u32) -> Self {
        match value {
            1 => PfrKeystoreUpdateOpt::WriteMemory,
            // 0 => KeyProvisioning, default for unknown values
            _ => PfrKeystoreUpdateOpt::KeyProvisioning,
        }
    }
}

/// Flash read margin setting.
///
/// Specifies the margin level used for flash read operations.
#[derive(Clone, Copy, Debug)]
pub enum FlashReadMargin {
    /// Normal read margin
    Normal = 0,
    /// User read margin
    User = 1,
    /// Factory read margin
    Factory = 2,
}

impl Display for FlashReadMargin {
    /// Format flash read margin as string representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlashReadMargin::Normal => write!(f, "NORMAL"),
            FlashReadMargin::User => write!(f, "USER"),
            FlashReadMargin::Factory => write!(f, "FACTORY"),
        }
    }
}

impl FlashReadMargin {
    #[must_use]
    pub fn parse(value: u32) -> Self {
        match value {
            1 => FlashReadMargin::User,
            2 => FlashReadMargin::Factory,
            // 0 => Normal, default is Normal for unknown values
            _ => FlashReadMargin::Normal, // Default to Normal for unknown values
        }
    }
}

/// Fuse program voltage setting.
///
/// Indicates the voltage level used for programming fuses.
#[derive(Clone, Copy, Debug)]
pub struct FuseProgramVoltage(bool);

impl Display for FuseProgramVoltage {
    /// Format fuse program voltage as string representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.0 {
            "Over Drive Voltage (2.5 V)"
        } else {
            "Normal Voltage (1.8 V)"
        };
        write!(f, "{state}")
    }
}

impl FuseProgramVoltage {
    #[must_use]
    pub fn parse(value: u32) -> Self {
        FuseProgramVoltage(value != 0)
    }
}

/// Secure Hardware Extension flash partition information.
///
/// Contains information about the SHE flash partition configuration.
#[derive(Clone, Copy, Debug)]
pub struct SHEFlashPartition {
    /// Maximum number of keys supported
    max_keys: u8,
    /// Flash size configuration
    flash_size: u8,
}

impl SHEFlashPartition {
    #[must_use]
    pub fn parse(value: u32) -> Self {
        SHEFlashPartition {
            max_keys: (value & 0x03) as u8,
            flash_size: ((value >> 8) & 0x03) as u8,
        }
    }
}

impl Display for SHEFlashPartition {
    /// Format SHE flash partition information as string representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let max_keys_str = match self.max_keys {
            0 => "0 Keys, CSEc disabled",
            1 => "max 5 Key",
            2 => "max 10 Keys",
            3 => "max 20 Keys",
            _ => "Unknown",
        };

        let flash_size_str = match self.flash_size {
            0 => "64kB",
            1 => "48kB",
            2 => "32kB",
            3 => "0kB",
            _ => "Unknown",
        };

        write!(f, "{flash_size_str} EEPROM with {max_keys_str}")
    }
}

/// Secure Hardware Extension boot mode information.
///
/// Contains information about the SHE boot mode configuration.
#[derive(Clone, Copy, Debug)]
pub struct SHEBootMode {
    /// Boot size
    size: u32,
    /// Boot mode
    mode: u8,
}

impl SHEBootMode {
    #[must_use]
    pub fn parse(value: u32) -> Self {
        SHEBootMode {
            size: value & 0x3FFF_FFFF,
            mode: ((value >> 30) & 0x03) as u8,
        }
    }
}

impl Display for SHEBootMode {
    /// Format SHE boot mode information as string representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode_str = match self.mode {
            0 => "Strict Boot",
            1 => "Serial Boot",
            2 => "Parallel Boot",
            3 => "Undefined",
            _ => "Unknown",
        };

        write!(
            f,
            "SHE Boot Mode: {} ({})\nSHE Boot Size: {} (0x{:X})",
            mode_str,
            self.mode,
            BinaryBytesOne(self.size / 8),
            self.size
        )
    }
}

/// Life cycle state information.
///
/// Indicates whether the device is in development or deployment life cycle.
#[derive(Clone, Copy, Debug)]
pub struct LifeCycleState(bool);

impl Display for LifeCycleState {
    /// Format life cycle state as string representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.0 {
            "development life cycle"
        } else {
            "deployment life cycle"
        };
        write!(f, "{state}")
    }
}
