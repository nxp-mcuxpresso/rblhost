// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use std::{io, time::Duration};

use crate::mboot::ResultComm;
use color_print::cstr;
use hidapi::{HidApi, HidDevice};
use log::{debug, info};
use std::fmt::Debug;

use super::{CommunicationError, Protocol, ProtocolOpen};

/// Report IDs for USB-HID protocol as per NXP documentation
mod report {
    /// Command packet from host to device
    pub const CMD_OUT: u8 = 0x01;
    /// Data packet from host to device
    pub const DATA_OUT: u8 = 0x02;
    /// Response packet from device to host
    pub const CMD_IN: u8 = 0x03;
    /// Data packet from device to host
    pub const DATA_IN: u8 = 0x04;
}

/// Maximum packet size for USB transfers
const MAX_PACKET_SIZE: usize = 1024;

#[derive(Debug)]
pub struct USBProtocol {
    interface: String,
    device: HidDevice,
    timeout_ms: i32,
    polling_interval: Duration,
}

impl ProtocolOpen for USBProtocol {
    fn open(identifier: &str) -> ResultComm<Self> {
        Self::open_with_options(identifier, 0, Duration::from_secs(5), Duration::from_millis(1))
    }

    fn open_with_options(
        identifier: &str,
        _baudrate: u32, // Not used for USB
        timeout: Duration,
        polling_interval: Duration,
    ) -> ResultComm<Self> {
        // Parse the identifier which can be in format "vid:pid" or a path
        let (vid, pid) = parse_usb_identifier(identifier)?;

        // Initialize HidApi
        let api =
            HidApi::new().map_err(|e| CommunicationError::ParseError(format!("Failed to initialize HID API: {e}")))?;

        // Find and open the device
        let device = api
            .open(vid, pid)
            .map_err(|e| CommunicationError::ParseError(format!("Failed to open USB device: {e}")))?;

        // Convert timeout to i32, clamping if necessary
        let timeout_ms = timeout.as_millis().try_into().unwrap_or(i32::MAX);

        let usb_protocol = USBProtocol {
            interface: identifier.to_owned(),
            device,
            timeout_ms,
            polling_interval,
        };

        info!(
            "Opened USB-HID device {} with {}ms timeout",
            usb_protocol.interface,
            timeout.as_millis()
        );

        Ok(usb_protocol)
    }
}

impl Protocol for USBProtocol {
    fn get_polling_interval(&self) -> Duration {
        self.polling_interval
    }

    fn get_timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.try_into().expect("negative timeout in USB"))
    }

    fn get_identifier(&self) -> &str {
        &self.interface
    }

    fn read(&mut self, bytes: usize) -> ResultComm<Vec<u8>> {
        let mut buf = vec![0u8; bytes];
        self.read_usb(&mut buf)?;
        Ok(buf)
    }
    fn write_packet_raw(&mut self, data: &[u8]) -> ResultComm<()> {
        // For USB-HID, we need to extract the command data from the UART framing
        // UART frame format: [5A, cmd_type, len_lsb, len_msb, crc_lsb, crc_msb, ...data...]
        if data.len() < 6 || data[0] != 0x5A {
            return Err(CommunicationError::InvalidHeader);
        }

        let cmd_type = data[1];
        let data_len = u16::from_le_bytes([data[2], data[3]]) as usize;

        if data.len() < 6 + data_len {
            return Err(CommunicationError::InvalidData);
        }

        // Extract the command data (without UART framing and CRC)
        // Skip the UART header (4 bytes) and CRC (2 bytes)
        let cmd_data = &data[6..6 + data_len];

        // Determine report ID based on packet type
        let report_id = match cmd_type {
            0xA4 => report::CMD_OUT,  // Command packet
            0xA5 => report::DATA_OUT, // Data packet
            _ => return Err(CommunicationError::InvalidHeader),
        };

        // Create a generic HID report
        let mut report = vec![0u8; 4 + cmd_data.len()]; // 4 bytes for header + data

        // Set report header
        report[0] = report_id;
        report[1] = 0x00; // Padding (should be 0)
        report[2] = (cmd_data.len() & 0xFF) as u8;
        report[3] = ((cmd_data.len() >> 8) & 0xFF) as u8;

        // Copy command data
        report[4..4 + cmd_data.len()].copy_from_slice(cmd_data);

        // Write the report
        self.write_usb(&report)?;

        Ok(())
    }
    //

    fn read_packet_raw(&mut self, _: u8) -> ResultComm<Vec<u8>> {
        // Read the initial response
        let mut report = vec![0u8; MAX_PACKET_SIZE];
        let size = self
            .device
            .read_timeout(&mut report, self.timeout_ms)
            .map_err(|e| CommunicationError::IOError(io::Error::other(e.to_string())))?;

        debug!("{}: Read {} bytes: {:02X?}", cstr!("<r!>RX"), size, &report[..size]);

        if size < 4 {
            return Err(CommunicationError::InvalidHeader);
        }

        // Extract report ID and packet length
        let report_id = report[0];
        let packet_length = u16::from_le_bytes([report[2], report[3]]) as usize;

        if packet_length == 0 {
            // error!(cstr!("<r!>RX</>: Data aborted by sender!"));
            return Err(CommunicationError::Aborted);
        }

        // Check if this is a command response (report ID 0x03)
        if report_id == report::CMD_IN {
            // For other command responses, extract the payload
            let mut response = Vec::new();

            // Extract the command tag and other fields
            response.extend_from_slice(&report[4..4 + packet_length]);

            debug!("Constructed response: {response:02X?}");

            return Ok(response);
        } else if report_id == report::DATA_IN {
            // Data packet - extract the data portion
            if size >= 4 + packet_length {
                return Ok(report[4..4 + packet_length].to_vec());
            }
        }

        // For other packet types, just return the data portion
        if size > 4 {
            Ok(report[4..size].to_vec())
        } else {
            Ok(Vec::new())
        }
    }
}

impl USBProtocol {
    fn read_usb(&mut self, buf: &mut [u8]) -> Result<(), io::Error> {
        match self.device.read(buf) {
            Ok(size) => {
                debug!("{}: Read {} bytes: {:02X?}", cstr!("<r!>RX"), size, &buf[..size]);
                Ok(())
            }
            Err(e) => Err(io::Error::other(e.to_string())),
        }
    }
    fn write_usb(&self, buf: &[u8]) -> Result<(), io::Error> {
        debug!("{}: {:02X?}", cstr!("<g!>TX"), buf);

        match self.device.write(buf) {
            Ok(written) => {
                // Platform-specific validation
                #[cfg(target_os = "windows")]
                {
                    // Windows HID might report different sizes due to report descriptors
                    // As long as write succeeded (written > 0), consider it successful
                    if written > 0 {
                        Ok(())
                    } else {
                        Err(io::Error::other("Failed to write to USB device"))
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // On other platforms, we expect the exact byte count
                    if written == buf.len() {
                        Ok(())
                    } else {
                        Err(io::Error::other(format!(
                            "Failed to write all bytes: wrote {} of {}",
                            written,
                            buf.len()
                        )))
                    }
                }
            }
            Err(e) => Err(io::Error::other(e.to_string())),
        }
    }
}

// Helper functions

fn parse_usb_identifier(identifier: &str) -> ResultComm<(u16, u16)> {
    // Check if the identifier contains a separator (either ':' or ',')
    if let Some(pos) = identifier.find([':', ',']) {
        let vid_str = &identifier[..pos];
        let pid_str = &identifier[pos + 1..];

        let vid = parse_number_string(vid_str)
            .map_err(|_| CommunicationError::ParseError(format!("Invalid VID: {vid_str}")))?;

        let pid = parse_number_string(pid_str)
            .map_err(|_| CommunicationError::ParseError(format!("Invalid PID: {pid_str}")))?;

        Ok((vid, pid))
    } else {
        // Try to parse as a single value (VID only)
        let vid = parse_number_string(identifier)
            .map_err(|_| CommunicationError::ParseError(format!("Invalid USB identifier: {identifier}")))?;

        // Use 0 as default PID, which will match any device with the specified VID
        Ok((vid, 0))
    }
}

/// Parse a number string that can be either decimal or hexadecimal
fn parse_number_string(s: &str) -> Result<u16, std::num::ParseIntError> {
    let trimmed = s.trim();

    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        // Hexadecimal with prefix
        u16::from_str_radix(&trimmed[2..], 16)
    } else if trimmed.chars().all(|c| c.is_ascii_hexdigit())
        && trimmed.len() > 2
        && trimmed.chars().any(|c| matches!(c, 'a'..='f' | 'A'..='F'))
    {
        // Hexadecimal without prefix (contains hex digits a-f)
        u16::from_str_radix(trimmed, 16)
    } else {
        // Try decimal first, then hexadecimal as fallback
        trimmed.parse::<u16>().or_else(|_| u16::from_str_radix(trimmed, 16))
    }
}
