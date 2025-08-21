// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use std::{
    fs::{File, OpenOptions},
    hint,
    io::{self, Read, Write},
    os::fd::AsRawFd,
    thread,
    time::{Duration, Instant},
};

use color_print::cstr;
use log::{debug, error, info, trace};

use super::DEFAULT_SLAVE;
use crate::mboot::{
    ResultComm,
    packets::{
        CRC_CHECK, Packet, PacketParse,
        ping::{Ping, PingResponse},
    },
    protocols::{ACK, ACK_ABORT, NACK, Protocol, ProtocolOpen},
};

use crate::CommunicationError;
use crate::parsers::parse_number;

#[derive(Debug)]
pub struct I2CProtocol {
    interface: String,
    device: File,
    slave_address: u8,
    timeout: Duration,
    polling_interval: Duration,
}

impl ProtocolOpen for I2CProtocol {
    fn open(identifier: &str) -> ResultComm<Self> {
        Self::open_with_options(identifier, 0, Duration::from_secs(5), Duration::from_millis(1))
    }

    fn open_with_options(
        identifier: &str,
        _baudrate: u32,
        timeout: Duration,
        polling_interval: Duration,
    ) -> ResultComm<Self> {
        // Check if identifier contains slave address
        let mut parts = identifier.split(':');
        let device_path = parts.next().unwrap();
        let (interface, slave_address) = match parts.next() {
            Some(num_str) => {
                trace!("num_str: {num_str}");
                let slave_address: u8 = parse_number(num_str).map_err(CommunicationError::ParseError)?;
                (format!("{device_path}:{slave_address:#02X}"), slave_address)
            }
            None => (identifier.to_owned(), DEFAULT_SLAVE),
        };

        if parts.next().is_some() {
            return Err(CommunicationError::InvalidData);
        }

        // Open the I2C device
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .open(device_path)
            .map_err(CommunicationError::FileError)?;

        // Set the slave address using ioctl
        // Note: This requires the i2c-dev kernel module to be loaded
        unsafe {
            let i2c_slave = 0x0703; // I2C_SLAVE ioctl command
            let result = libc::ioctl(device.as_raw_fd(), i2c_slave, libc::c_ulong::from(slave_address));
            if result < 0 {
                return Err(io::Error::last_os_error().into());
            }
        }

        let mut device = I2CProtocol {
            interface,
            device,
            slave_address,
            timeout,
            polling_interval,
        };

        info!(
            "Opened I2C device {} with slave address 0x{:02X} with {}ms timeout",
            device_path,
            slave_address,
            timeout.as_millis()
        );

        // Test connection with ping
        device.ping()?;

        Ok(device)
    }
}

impl Protocol for I2CProtocol {
    fn get_timeout(&self) -> Duration {
        self.timeout
    }

    fn get_polling_interval(&self) -> Duration {
        self.polling_interval
    }

    fn get_identifier(&self) -> &str {
        &self.interface
    }

    fn read(&mut self, bytes: usize) -> ResultComm<Vec<u8>> {
        let mut buf = vec![0u8; bytes];
        self.read_static(&mut buf)?;
        Ok(buf)
    }

    fn write_packet_raw(&mut self, data: &[u8]) -> ResultComm<()> {
        self.write(data)?;
        self.read_ack()?;
        Ok(())
    }

    fn read_packet_raw(&mut self, packet_code: u8) -> ResultComm<Vec<u8>> {
        let mut data = self.read(2)?;

        if data[..2] != [0x5a, packet_code] {
            return Err(CommunicationError::InvalidHeader);
        }

        data.extend(self.read(2)?);
        let length = u16::from_le_bytes(data[2..4].try_into().or(Err(CommunicationError::InvalidHeader))?);

        let crc = u16::from_le_bytes(self.read(2)?.try_into().or(Err(CommunicationError::InvalidHeader))?);

        // reading command part
        data.extend(self.read(length as usize)?);

        self.send_ack()?;

        if CRC_CHECK.checksum(&data) != crc {
            return Err(CommunicationError::InvalidCrc);
        }

        if length == 0 {
            error!(cstr!("<r!>RX</>: Data aborted by sender!"));
            return Err(CommunicationError::Aborted);
        }

        let data_slice = &data[4..];
        Ok(data_slice.to_vec())
    }
}

impl I2CProtocol {
    fn read_static(&mut self, buf: &mut [u8]) -> Result<(), io::Error> {
        self.device.read_exact(buf)?;
        debug!("{}: {buf:02X?}", cstr!("<r!>RX"));
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), io::Error> {
        debug!("{}: {buf:02X?}", cstr!("<g!>TX"));
        self.device.write_all(buf)
    }

    fn ping(&mut self) -> ResultComm<PingResponse> {
        trace!("Pinging device with slave address 0x{:02X}", self.slave_address);
        self.write(&[0x5a, Ping::get_code()])?;

        // After power cycle, MBoot v3.0+ may respond with leading dummy data
        // We need to read data until we find the frame start byte (0x5A)
        const MAX_PING_RESPONSE_DUMMY_BYTES: usize = 50;
        let mut start_byte = [0u8; 1];

        for i in 0..MAX_PING_RESPONSE_DUMMY_BYTES {
            if let Err(e) = self.device.read_exact(&mut start_byte) {
                return Err(CommunicationError::IOError(e));
            }

            if start_byte[0] == 0x5A {
                trace!("FRAME_START_BYTE received in {}. attempt.", i + 1);
                break;
            }

            trace!("Received dummy byte: 0x{:02X}", start_byte[0]);

            if i == MAX_PING_RESPONSE_DUMMY_BYTES - 1 {
                return Err(CommunicationError::InvalidHeader);
            }
        }

        // Read frame type (should be PingResponse code)
        let mut frame_type = [0u8; 1];
        self.device.read_exact(&mut frame_type)?;

        if frame_type[0] != PingResponse::get_code() {
            return Err(CommunicationError::InvalidHeader);
        }

        // Read the rest of the response (8 bytes)
        let mut response_data = [0u8; 8];
        self.device.read_exact(&mut response_data)?;

        // Combine all parts for CRC check and debug output
        let mut buf = [0u8; 10];
        buf[0] = start_byte[0];
        buf[1] = frame_type[0];
        buf[2..].copy_from_slice(&response_data);

        debug!("{}: {buf:02X?}", cstr!("<r!>RX"));

        let crc = u16::from_le_bytes(buf[8..].try_into().or(Err(CommunicationError::InvalidHeader))?);

        if CRC_CHECK.checksum(&buf[..8]) != crc {
            return Err(CommunicationError::InvalidCrc);
        }

        let res = PingResponse::parse(&buf)?;
        Ok(res)
    }

    fn send_ack(&mut self) -> Result<(), std::io::Error> {
        trace!("Sending ACK");
        self.write(&[0x5a, ACK])
    }

    fn read_ack(&mut self) -> ResultComm<()> {
        let timeout = self.get_timeout();
        let polling_interval = self.get_polling_interval();
        let start = Instant::now();
        let mut buf = [0u8; 2];

        trace!(
            "Reading ACK with timeout {}ms and polling interval {}ms",
            timeout.as_millis(),
            polling_interval.as_millis()
        );

        while start.elapsed() < timeout {
            // helping the CPU know we're busy waiting
            hint::spin_loop();
            thread::sleep(polling_interval);

            if self.read_static(&mut buf).is_ok() {
                // If we get 0x00, it means the device is busy, so we should continue polling
                if buf[0] == 0x00 {
                    trace!("Device busy (received 0x00), continuing to poll");
                    continue;
                }

                // Check for the frame start marker
                if buf[0] != 0x5a {
                    trace!("Invalid frame start marker: 0x{:02X}, continuing to poll", buf[0]);
                    continue;
                }

                return match buf[1] {
                    ACK => Ok(()),
                    NACK => Err(CommunicationError::NACKSent),
                    ACK_ABORT => Err(CommunicationError::Aborted),
                    _ => {
                        trace!("Invalid ACK code: 0x{:02X}, continuing to poll", buf[1]);
                        continue;
                    }
                };
            }
        }

        Err(CommunicationError::Timeout)
    }
}
