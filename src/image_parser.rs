// Copyright 2025-2026 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use std::{fs::File, io::Read, path::Path};

use mboot::CommunicationError;
use srec_rs::SRecord;

const SPARSE_MAGIC: u32 = 0xED26_FF3A;
const SPARSE_HEADER_SIZE: usize = 28;
const SPARSE_CHUNK_HEADER_SIZE: usize = 12;
const SPARSE_CHUNK_RAW: u16 = 0xCAC1;
const SPARSE_CHUNK_FILL: u16 = 0xCAC2;
const SPARSE_CHUNK_DONT_CARE: u16 = 0xCAC3;
const SPARSE_CHUNK_CRC32: u16 = 0xCAC4;
const SPARSE_CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);

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

        // Android SPARSE images are binary, so detect them by magic before trying S-record
        if Self::is_sparse_format(file_path) {
            return Self::parse_sparse(file_path);
        }

        if Self::is_ihex_format(file_path) {
            return Self::parse_ihex(file_path);
        }

        // Try to parse as S-record
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

    /// Check if a file starts with the Android SPARSE image magic
    pub fn is_sparse_format(file_path: &str) -> bool {
        let mut magic = [0u8; 4];
        File::open(file_path)
            .and_then(|mut file| file.read_exact(&mut magic))
            .is_ok_and(|()| u32::from_le_bytes(magic) == SPARSE_MAGIC)
    }

    /// Parse Android SPARSE image file
    fn parse_sparse(file_path: &str) -> Result<Vec<ImageSegment>, CommunicationError> {
        let mut file = File::open(file_path).map_err(CommunicationError::FileError)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(CommunicationError::FileError)?;
        Self::parse_sparse_bytes(&data)
    }

    /// Convert Android SPARSE image data to segments
    ///
    /// RAW chunks are written as data and FILL chunks as their repeated 4-byte value.
    /// Consecutive RAW chunks, and consecutive FILL chunks with the same value, form one
    /// segment. DONT_CARE chunks are gaps that are not written. Segment addresses are
    /// offsets from the start of the image.
    fn parse_sparse_bytes(data: &[u8]) -> Result<Vec<ImageSegment>, CommunicationError> {
        let read_u16 = |offset: usize| -> Result<u16, CommunicationError> {
            data.get(offset..offset + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .ok_or(CommunicationError::InvalidData)
        };
        let read_u32 = |offset: usize| -> Result<u32, CommunicationError> {
            data.get(offset..offset + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or(CommunicationError::InvalidData)
        };

        if read_u32(0)? != SPARSE_MAGIC
            || usize::from(read_u16(8)?) != SPARSE_HEADER_SIZE
            || usize::from(read_u16(10)?) != SPARSE_CHUNK_HEADER_SIZE
        {
            return Err(CommunicationError::InvalidData);
        }
        let block_size = read_u32(12)?;
        let total_blocks = read_u32(16)?;
        let total_chunks = read_u32(20)?;
        let image_checksum = read_u32(24)?;
        if block_size == 0 || !block_size.is_power_of_two() {
            return Err(CommunicationError::InvalidData);
        }
        let image_size = block_size
            .checked_mul(total_blocks)
            .ok_or(CommunicationError::InvalidData)?;

        let mut segments: Vec<ImageSegment> = Vec::new();
        // Kind of the last segment, so only chunks of the same kind are merged into it
        let mut last_chunk: Option<(u16, [u8; 4])> = None;
        let mut crc_values = Vec::new();
        let mut address: u32 = 0;
        let mut offset = SPARSE_HEADER_SIZE;

        for _ in 0..total_chunks {
            let chunk_type = read_u16(offset)?;
            let chunk_blocks = read_u32(offset + 4)?;
            let total_size = usize::try_from(read_u32(offset + 8)?).map_err(|_| CommunicationError::InvalidData)?;
            let body_size = total_size
                .checked_sub(SPARSE_CHUNK_HEADER_SIZE)
                .ok_or(CommunicationError::InvalidData)?;
            let body_start = offset + SPARSE_CHUNK_HEADER_SIZE;
            let body = data
                .get(body_start..body_start + body_size)
                .ok_or(CommunicationError::InvalidData)?;
            let chunk_size = chunk_blocks
                .checked_mul(block_size)
                .ok_or(CommunicationError::InvalidData)?;
            let chunk_len = usize::try_from(chunk_size).map_err(|_| CommunicationError::InvalidData)?;

            let chunk_data = match chunk_type {
                SPARSE_CHUNK_RAW => {
                    if body.len() != chunk_len {
                        return Err(CommunicationError::InvalidData);
                    }
                    Some((body.to_vec(), [0; 4]))
                }
                SPARSE_CHUNK_FILL => {
                    let value: [u8; 4] = body.try_into().map_err(|_| CommunicationError::InvalidData)?;
                    Some((value.repeat(chunk_len / 4), value))
                }
                SPARSE_CHUNK_DONT_CARE => None,
                SPARSE_CHUNK_CRC32 => {
                    let value: [u8; 4] = body.try_into().map_err(|_| CommunicationError::InvalidData)?;
                    crc_values.push(u32::from_le_bytes(value));
                    None
                }
                _ => return Err(CommunicationError::InvalidData),
            };

            match chunk_data {
                Some((bytes, fill_value)) => {
                    let kind = (chunk_type, fill_value);
                    match segments.last_mut() {
                        Some(segment) if last_chunk == Some(kind) => segment.data.extend_from_slice(&bytes),
                        _ => segments.push(ImageSegment { address, data: bytes }),
                    }
                    last_chunk = Some(kind);
                }
                None => last_chunk = None,
            }

            if chunk_type != SPARSE_CHUNK_CRC32 {
                address = address.checked_add(chunk_size).ok_or(CommunicationError::InvalidData)?;
            }
            offset = body_start + body_size;
        }

        if address != image_size {
            return Err(CommunicationError::InvalidData);
        }

        if image_checksum != 0 {
            crc_values.push(image_checksum);
        }
        if !crc_values.is_empty() {
            // The checksum covers the whole expanded image, with DONT_CARE areas as zeros
            let mut image = vec![0u8; usize::try_from(image_size).map_err(|_| CommunicationError::InvalidData)?];
            for segment in &segments {
                let start = usize::try_from(segment.address).map_err(|_| CommunicationError::InvalidData)?;
                image[start..start + segment.data.len()].copy_from_slice(&segment.data);
            }
            let calculated = SPARSE_CRC.checksum(&image);
            if crc_values.iter().any(|&crc| crc != calculated) {
                return Err(CommunicationError::InvalidCrc);
            }
        }

        Ok(segments)
    }

    /// Check if a file starts with an Intel HEX record
    pub fn is_ihex_format(file_path: &str) -> bool {
        std::fs::read_to_string(file_path).is_ok_and(|content| {
            content
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .is_some_and(|line| Self::parse_ihex_record(line).is_ok())
        })
    }

    /// Parse Intel HEX file
    fn parse_ihex(file_path: &str) -> Result<Vec<ImageSegment>, CommunicationError> {
        let content = std::fs::read_to_string(file_path).map_err(CommunicationError::FileError)?;
        Self::parse_ihex_str(&content)
    }

    /// Decode one Intel HEX record line into its bytes and verify the length and checksum
    fn parse_ihex_record(line: &str) -> Result<Vec<u8>, CommunicationError> {
        let digits = line.strip_prefix(':').ok_or(CommunicationError::InvalidData)?;
        if digits.len() % 2 != 0 {
            return Err(CommunicationError::InvalidData);
        }
        let bytes = (0..digits.len())
            .step_by(2)
            .map(|i| {
                digits
                    .get(i..i + 2)
                    .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                    .ok_or(CommunicationError::InvalidData)
            })
            .collect::<Result<Vec<u8>, _>>()?;
        // byte count, 2 address bytes, record type, data and checksum
        if bytes.len() < 5 || bytes.len() != 5 + usize::from(bytes[0]) {
            return Err(CommunicationError::InvalidData);
        }
        if bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
            return Err(CommunicationError::InvalidCrc);
        }
        Ok(bytes)
    }

    /// Convert Intel HEX content to segments
    ///
    /// Data records are placed using the extended segment/linear address records, contiguous
    /// data forms one segment. Start address records are ignored.
    fn parse_ihex_str(content: &str) -> Result<Vec<ImageSegment>, CommunicationError> {
        const DATA: u8 = 0x00;
        const END_OF_FILE: u8 = 0x01;
        const EXTENDED_SEGMENT_ADDRESS: u8 = 0x02;
        const START_SEGMENT_ADDRESS: u8 = 0x03;
        const EXTENDED_LINEAR_ADDRESS: u8 = 0x04;
        const START_LINEAR_ADDRESS: u8 = 0x05;

        let mut chunks: Vec<ImageSegment> = Vec::new();
        let mut base_address: u32 = 0;
        for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let record = Self::parse_ihex_record(line)?;
            let offset = u32::from(u16::from_be_bytes([record[1], record[2]]));
            let data = &record[4..record.len() - 1];
            match record[3] {
                DATA if data.is_empty() => {}
                DATA => {
                    let address = base_address
                        .checked_add(offset)
                        .ok_or(CommunicationError::InvalidData)?;
                    chunks.push(ImageSegment {
                        address,
                        data: data.to_vec(),
                    });
                }
                END_OF_FILE => break,
                EXTENDED_SEGMENT_ADDRESS | EXTENDED_LINEAR_ADDRESS => {
                    let value: [u8; 2] = data.try_into().map_err(|_| CommunicationError::InvalidData)?;
                    let shift = if record[3] == EXTENDED_LINEAR_ADDRESS { 16 } else { 4 };
                    base_address = u32::from(u16::from_be_bytes(value)) << shift;
                }
                START_SEGMENT_ADDRESS | START_LINEAR_ADDRESS => {}
                _ => return Err(CommunicationError::InvalidData),
            }
        }

        chunks.sort_by_key(|chunk| chunk.address);
        let mut segments: Vec<ImageSegment> = Vec::new();
        for chunk in chunks {
            if let Some(last) = segments.last_mut() {
                let last_end = u64::from(last.address) + last.data.len() as u64;
                if u64::from(chunk.address) < last_end {
                    // overlapping data records
                    return Err(CommunicationError::InvalidData);
                }
                if u64::from(chunk.address) == last_end {
                    last.data.extend_from_slice(&chunk.data);
                    continue;
                }
            }
            segments.push(chunk);
        }
        Ok(segments)
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

    /// Two segments, the first one crossing a 64 KiB boundary, with a start linear address record
    const IHEX: &str = "\
:10FFF000000102030405060708090A0B0C0D0E0F89
:020000040001F9
:10000000101112131415161718191A1B1C1D1E1F78
:080010002021222324252627CC
:020000042800D2
:08000000A5A5A5A5A5A5A5A5D0
:0400000528000000CF
:00000001FF
";

    #[test]
    fn test_parse_ihex() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(IHEX.as_bytes()).unwrap();
        let path = temp_file.path().to_str().unwrap();

        assert!(ImageParser::is_ihex_format(path));
        assert!(!ImageParser::is_sparse_format(path));
        let segments = ImageParser::parse_file(path).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].address, 0xFFF0);
        assert_eq!(segments[0].data, (0..40).collect::<Vec<u8>>());
        assert_eq!(segments[1].address, 0x2800_0000);
        assert_eq!(segments[1].data, vec![0xA5; 8]);
    }

    #[test]
    fn test_parse_ihex_extended_segment_address() {
        let segments =
            ImageParser::parse_ihex_str(":020000021000EC\r\n:0400100001020304E2\r\n:00000001FF\r\n").unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].address, 0x10010);
        assert_eq!(segments[0].data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_ihex_rejects_bad_checksum() {
        assert!(matches!(
            ImageParser::parse_ihex_str(":0400000001020304F1\n:00000001FF\n"),
            Err(CommunicationError::InvalidCrc)
        ));
    }

    #[test]
    fn test_parse_ihex_rejects_overlapping_records() {
        assert!(ImageParser::parse_ihex_str(":0400000001020304F2\n:020002000506F1\n:00000001FF\n").is_err());
    }

    #[test]
    fn test_binary_is_not_ihex() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&[0x3A, 0x00, 0xFF, 0x10]).unwrap();
        assert!(!ImageParser::is_ihex_format(temp_file.path().to_str().unwrap()));
    }

    fn sparse_header(block_size: u32, total_blocks: u32, total_chunks: u32, checksum: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&SPARSE_MAGIC.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&28u16.to_le_bytes());
        data.extend_from_slice(&12u16.to_le_bytes());
        data.extend_from_slice(&block_size.to_le_bytes());
        data.extend_from_slice(&total_blocks.to_le_bytes());
        data.extend_from_slice(&total_chunks.to_le_bytes());
        data.extend_from_slice(&checksum.to_le_bytes());
        data
    }

    fn sparse_chunk(chunk_type: u16, chunk_blocks: u32, body: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&chunk_type.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&chunk_blocks.to_le_bytes());
        data.extend_from_slice(&u32::try_from(12 + body.len()).unwrap().to_le_bytes());
        data.extend_from_slice(body);
        data
    }

    #[test]
    fn test_parse_sparse_raw_with_crc_chunk() {
        let raw: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let crc = SPARSE_CRC.checksum(&raw);
        let mut data = sparse_header(4096, 2, 2, 0);
        data.extend(sparse_chunk(SPARSE_CHUNK_RAW, 2, &raw));
        data.extend(sparse_chunk(SPARSE_CHUNK_CRC32, 0, &crc.to_le_bytes()));

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&data).unwrap();
        let path = temp_file.path().to_str().unwrap();

        assert!(ImageParser::is_sparse_format(path));
        let segments = ImageParser::parse_file(path).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].address, 0);
        assert_eq!(segments[0].data, raw);
    }

    #[test]
    fn test_parse_sparse_fill_and_dont_care() {
        let raw = vec![0xA5u8; 4096];
        let fill = [0x11, 0x22, 0x33, 0x44];
        let mut image = raw.clone();
        image.extend(vec![0u8; 4096]);
        image.extend(fill.repeat(2048));
        image.extend(raw.clone());
        let mut data = sparse_header(4096, 5, 5, SPARSE_CRC.checksum(&image));
        data.extend(sparse_chunk(SPARSE_CHUNK_RAW, 1, &raw));
        data.extend(sparse_chunk(SPARSE_CHUNK_DONT_CARE, 1, &[]));
        data.extend(sparse_chunk(SPARSE_CHUNK_FILL, 1, &fill));
        data.extend(sparse_chunk(SPARSE_CHUNK_FILL, 1, &fill));
        data.extend(sparse_chunk(SPARSE_CHUNK_RAW, 1, &raw));

        let segments = ImageParser::parse_sparse_bytes(&data).unwrap();
        assert_eq!(segments.len(), 3);
        assert_eq!((segments[0].address, segments[0].data.len()), (0, 4096));
        assert_eq!(segments[1].address, 0x2000);
        assert_eq!(segments[1].data, fill.repeat(2048));
        assert_eq!(segments[2].address, 0x4000);
        assert_eq!(segments[2].data, raw);
    }

    #[test]
    fn test_parse_sparse_rejects_bad_crc() {
        let raw = vec![0x5Au8; 4096];
        let mut data = sparse_header(4096, 1, 2, 0);
        data.extend(sparse_chunk(SPARSE_CHUNK_RAW, 1, &raw));
        data.extend(sparse_chunk(SPARSE_CHUNK_CRC32, 0, &0xDEAD_BEEFu32.to_le_bytes()));

        assert!(matches!(
            ImageParser::parse_sparse_bytes(&data),
            Err(CommunicationError::InvalidCrc)
        ));
    }

    #[test]
    fn test_parse_sparse_rejects_truncated_chunk() {
        let mut data = sparse_header(4096, 1, 1, 0);
        data.extend(sparse_chunk(SPARSE_CHUNK_RAW, 1, &[0u8; 100]));

        assert!(ImageParser::parse_sparse_bytes(&data).is_err());
    }
}
