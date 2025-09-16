// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use std::{fs::File, io::Read, path::Path};

use mboot::CommunicationError;
use srec_rs::SRecord;

#[derive(Debug, Clone)]
pub struct ImageSegment {
    pub address: u32,
    pub data: Vec<u8>,
}

pub struct ImageParser;

impl ImageParser {
    /// Parse an image file and return segments
    pub fn parse_file(file_path: &str) -> Result<Vec<ImageSegment>, CommunicationError> {
        if !Path::new(file_path).is_file() {
            return Err(CommunicationError::InvalidData);
        }

        // Try to parse as S-record first
        if let Ok(segments) = Self::parse_srec(file_path)
            && !segments.is_empty()
        {
            return Ok(segments);
        }

        // Fall back to binary format
        Self::parse_binary(file_path)
    }

    /// Parse S-record format file using srec-rs
    fn parse_srec(file_path: &str) -> Result<Vec<ImageSegment>, CommunicationError> {
        let file = File::open(file_path).map_err(CommunicationError::FileError)?;

        const MAX_BUFFER_SIZE: u32 = 0x4000000; // 64MB
        let srec = SRecord::<MAX_BUFFER_SIZE>::from_srec(file).map_err(|_| CommunicationError::InvalidData)?;

        let mut segments = Vec::new();

        // Get data layout from srec-rs (returns Vec<(Address, &[u8])>)
        let data_layout = srec.get_data_layout();

        // Convert data layout to segments
        for (address, data_slice) in data_layout {
            let start_addr = match address {
                srec_rs::Address::Address16(addr) => u32::from(addr),
                srec_rs::Address::Address24(addr) | srec_rs::Address::Address32(addr) => addr,
            };

            segments.push(ImageSegment {
                address: start_addr,
                data: data_slice.to_vec(),
            });
        }

        Ok(segments)
    }

    /// Parse binary format file
    fn parse_binary(file_path: &str) -> Result<Vec<ImageSegment>, CommunicationError> {
        let mut file = File::open(file_path).map_err(CommunicationError::FileError)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(CommunicationError::FileError)?;

        // For binary files, assume they start at address 0x0
        Ok(vec![ImageSegment { address: 0x0, data }])
    }

    /// Parse S-record with additional information extraction
    pub fn parse_srec_with_info(file_path: &str) -> Result<(Vec<ImageSegment>, SrecInfo), CommunicationError> {
        let file = File::open(file_path).map_err(CommunicationError::FileError)?;

        const MAX_BUFFER_SIZE: u32 = 0x4000000; // 64MB
        let srec = SRecord::<MAX_BUFFER_SIZE>::from_srec(file).map_err(|_| CommunicationError::InvalidData)?;

        // Extract segments using the same logic as parse_srec
        let segments = Self::parse_srec(file_path)?;

        // Extract additional info
        let info = SrecInfo {
            header: srec.get_header().map(std::string::ToString::to_string),
            data_length: srec.get_data_length(),
            memory_regions: srec.get_memory_layout().len(),
        };

        Ok((segments, info))
    }

    /// Check if a file appears to be in S-record format
    pub fn is_srec_format(file_path: &str) -> bool {
        if let Ok(mut file) = File::open(file_path) {
            let mut content = String::new();
            if file.read_to_string(&mut content).is_ok() {
                return content.lines().any(|line| {
                    let line = line.trim();
                    line.starts_with('S') && line.len() >= 6
                });
            }
        }
        false
    }
}

/// Additional information extracted from S-record files
#[derive(Debug, Default)]
pub struct SrecInfo {
    pub header: Option<String>,
    pub data_length: usize,
    pub memory_regions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_is_srec_format() {
        // Create a temporary S-record file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "S00F000068656C6C6F202020202000003C").unwrap();
        writeln!(
            temp_file,
            "S11F00007C0802A6900100049421FFF07C6C1B787C8C23784E800020E8010010"
        )
        .unwrap();
        writeln!(temp_file, "S5030001FB").unwrap();
        writeln!(temp_file, "S9030000FC").unwrap();

        assert!(ImageParser::is_srec_format(temp_file.path().to_str().unwrap()));
    }

    #[test]
    fn test_parse_binary() {
        // Create a temporary binary file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&[0x01, 0x02, 0x03, 0x04]).unwrap();

        let segments = ImageParser::parse_binary(temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].address, 0x0);
        assert_eq!(segments[0].data, vec![0x01, 0x02, 0x03, 0x04]);
    }
}
