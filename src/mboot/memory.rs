// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! McuBoot Memory Information Handling
//!
//! This module provides structures and functionality for handling memory-related information
//! returned by McuBoot devices. It includes parsing and display of reserved memory regions
//! and external memory attributes that can be queried from the target device.
//!
//! The memory information is typically obtained through [`CommandTag::GetProperty`][`super::tags::command::CommandTag::GetProperty`] commands and provides
//! details about memory layout, reserved regions, and external memory configurations.
//! This information is essential for understanding the target device's memory map and
//! constraints when performing memory operations.

use std::fmt::Display;

use super::formatters::BinaryBytesOne;

/// External memory property tag constants
///
/// These constants define the bit flags used to indicate which external memory
/// properties are available in the external memory attributes data structure.
/// Each flag corresponds to a specific property field in the response data.
pub mod ext_mem_prop_tags {
    /// Start address property is available
    pub const START_ADDRESS: u32 = 0x00000001;
    /// Size in kilobytes property is available
    pub const SIZE_IN_KBYTES: u32 = 0x00000002;
    /// Page size property is available
    pub const PAGE_SIZE: u32 = 0x00000004;
    /// Sector size property is available
    pub const SECTOR_SIZE: u32 = 0x00000008;
    /// Block size property is available
    pub const BLOCK_SIZE: u32 = 0x00000010;
}

pub mod mem_id {
    /// Internal RAM/FLASH (Used for the PRINCE configuration)
    pub const INTERNAL_MEMORY: u32 = 0;
    /// Quad SPI Memory 0
    pub const QUAD_SPI0: u32 = 1;
    /// Nonvolatile information register 0 (only used by SB loader)
    pub const IFR: u32 = 4;
    /// Nonvolatile information register 0 (only used by SB loader)
    pub const FUSE: u32 = 4;
    /// SEMC NOR Memory
    pub const SEMC_NOR: u32 = 8;
    /// Flex SPI NOR Memory
    pub const FLEX_SPI_NOR: u32 = 9;
    /// SPIFI NOR Memory
    pub const SPIFI_NOR: u32 = 10;
    /// Execute-Only region on internal Flash
    pub const FLASH_EXEC_ONLY: u32 = 16;
    /// SEMC NAND Memory
    pub const SEMC_NAND: u32 = 256;
    /// SPI NAND Memory
    pub const SPI_NAND: u32 = 257;
    /// SPI NOR/EEPROM Memory
    pub const SPI_NOR_EEPROM: u32 = 272;
    /// I2C NOR/EEPROM Memory
    pub const I2C_NOR_EEPROM: u32 = 273;
    /// eSD/SD/SDHC/SDXC Memory Card
    pub const SD_CARD: u32 = 288;
    /// MMC/eMMC Memory Card
    pub const MMC_CARD: u32 = 289;
}

/// Reserved memory regions information
///
/// Represents a collection of memory regions that are reserved and should not be
/// used for general memory operations. These regions typically contain bootloader
/// code, configuration data, or other critical system information that must be
/// preserved during memory operations.
#[derive(Clone, Debug)]
pub struct ReservedRegions {
    /// Array of (`start_address`, `end_address`) pairs defining reserved regions
    regions: Box<[(u32, u32)]>,
}

impl Display for ReservedRegions {
    /// Formats the reserved regions for display
    ///
    /// Displays each reserved region with its index, start address, end address,
    /// and total size in a human-readable format.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, (start, end)) in self.regions.iter().enumerate() {
            writeln!(
                f,
                "    Region {index}: {start:#010X} - {end:#010X}; Total Size: {}",
                BinaryBytesOne(end - start + 1)
            )?;
        }
        Ok(())
    }
}

impl ReservedRegions {
    /// Parses reserved regions data from u32 array
    ///
    /// Takes an array of u32 values where each pair represents a reserved region
    /// with start and end addresses. The array length must be even as each region
    /// requires exactly two values.
    ///
    /// # Arguments
    /// * `data` - Array of u32 values containing (start, end) pairs
    ///
    /// # Returns
    /// A new [`ReservedRegions`] instance containing the parsed regions
    ///
    /// # Panics
    /// Panics if the data array length is odd, as this indicates malformed data
    #[must_use]
    pub fn parse(data: &[u32]) -> Self {
        assert!(data.len() % 2 == 0, "reserved regions value is not odd");
        let regions = data.chunks(2).map(|region| (region[0], region[1])).collect();
        ReservedRegions { regions }
    }
}

/// External memory attributes information
///
/// Represents the properties and characteristics of external memory devices
/// that can be configured and used by the bootloader. This includes information
/// such as memory size, addressing, and block/sector organization.
#[derive(Clone, Copy, Debug)]
pub struct ExternalMemoryAttributes {
    /// Starting address of the external memory device
    start_address: Option<u32>,
    /// Total size of the external memory device, in KiB
    total_size: Option<u32>,
    /// Page size for programming operations
    page_size: Option<u32>,
    /// Sector size for erase operations
    sector_size: Option<u32>,
    /// Block size for bulk operations
    block_size: Option<u32>,
}

impl ExternalMemoryAttributes {
    /// Parses external memory attributes from u32 array
    ///
    /// Takes an array of u32 values where the first value contains flags indicating
    /// which properties are present, followed by the actual property values in a
    /// specific order. The presence of each property is determined by the corresponding
    /// flag bit in the first value.
    ///
    /// # Arguments
    /// * `data` - Array of u32 values containing flags and property values
    ///
    /// # Returns
    /// A new [`ExternalMemoryAttributes`] instance with parsed properties
    ///
    /// # Data Format
    /// -`data[0]`: Property flags (combination of [`ext_mem_prop_tags`] constants)
    /// -`data[1]`: Start address (if [`ext_mem_prop_tags::START_ADDRESS`] flag is set)
    /// -`data[2]`: Size in kilobytes (if [`ext_mem_prop_tags::SIZE_IN_KBYTES`] flag is set)
    /// -`data[3]`: Page size in bytes (if [`ext_mem_prop_tags::PAGE_SIZE`] flag is set)
    /// -`data[4]`: Sector size in bytes (if [`ext_mem_prop_tags::SECTOR_SIZE`] flag is set)
    /// -`data[5]`: Block size in bytes (if [`ext_mem_prop_tags::BLOCK_SIZE`] flag is set)
    #[must_use]
    pub fn parse(data: &[u32]) -> Self {
        let value = data[0];
        let start_address = if value & ext_mem_prop_tags::START_ADDRESS != 0 {
            Some(data[1])
        } else {
            None
        };
        let total_size = if value & ext_mem_prop_tags::SIZE_IN_KBYTES != 0 {
            Some(data[2])
        } else {
            None
        };
        let page_size = if value & ext_mem_prop_tags::PAGE_SIZE != 0 {
            Some(data[3])
        } else {
            None
        };
        let sector_size = if value & ext_mem_prop_tags::SECTOR_SIZE != 0 {
            Some(data[4])
        } else {
            None
        };
        let block_size = if value & ext_mem_prop_tags::BLOCK_SIZE != 0 {
            Some(data[5])
        } else {
            None
        };
        ExternalMemoryAttributes {
            start_address,
            total_size,
            page_size,
            sector_size,
            block_size,
        }
    }
}

impl Display for ExternalMemoryAttributes {
    /// Formats the external memory attributes for display
    ///
    /// Displays each available attribute with its name and value in a
    /// human-readable format. Only attributes that are present (not None)
    /// are displayed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(start_address) = self.start_address {
            write!(f, "Start Address: {start_address:#010X}")?;
        }
        if let Some(total_size) = self.total_size {
            write!(f, "Total Size:    {}", BinaryBytesOne(u64::from(total_size) * 1024))?;
        }
        if let Some(page_size) = self.page_size {
            write!(f, "Page Size:     {}", BinaryBytesOne(page_size))?;
        }
        if let Some(sector_size) = self.sector_size {
            write!(f, "Sector Size:   {}", BinaryBytesOne(sector_size))?;
        }
        if let Some(block_size) = self.block_size {
            write!(f, "Block Size:    {}", BinaryBytesOne(block_size))?;
        }
        Ok(())
    }
}
