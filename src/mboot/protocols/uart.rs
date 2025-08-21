// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use std::{
    io, thread,
    time::{Duration, Instant},
};

use color_print::cstr;
use log::{debug, error, info, trace};

use crate::mboot::{
    ResultComm,
    packets::{
        CRC_CHECK, Packet, PacketParse,
        ping::{Ping, PingResponse},
    },
    protocols::{ACK, ACK_ABORT, NACK},
};

use super::{CommunicationError, Protocol, ProtocolOpen};

#[derive(Debug)]
pub struct UARTProtocol {
    interface: String,
    port: Box<dyn serialport::SerialPort>,
    polling_interval: Duration,
}

impl ProtocolOpen for UARTProtocol {
    fn open(identifier: &str) -> ResultComm<Self> {
        Self::open_with_options(identifier, 57600, Duration::from_secs(5), Duration::from_millis(1))
    }

    fn open_with_options(
        identifier: &str,
        baudrate: u32,
        timeout: Duration,
        polling_interval: Duration,
    ) -> ResultComm<Self> {
        let s = serialport::new(identifier, baudrate).timeout(timeout).open()?;

        let mut device = UARTProtocol {
            interface: identifier.to_owned(),
            port: s,
            polling_interval,
        };

        info!(
            "Opened UART device {} at {} baud with {}ms timeout",
            device.interface,
            baudrate,
            timeout.as_millis()
        );

        device.ping()?;
        Ok(device)
    }
}

impl Protocol for UARTProtocol {
    fn get_polling_interval(&self) -> Duration {
        self.polling_interval
    }

    fn get_timeout(&self) -> Duration {
        self.port.timeout()
    }

    fn get_identifier(&self) -> &str {
        &self.interface
    }

    fn read(&mut self, bytes: usize) -> ResultComm<Vec<u8>> {
        let mut buf = vec![0u8; bytes];
        // ngl it's really cool that this is just provided by std::io trait
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

impl UARTProtocol {
    fn read_static(&mut self, buf: &mut [u8]) -> Result<(), io::Error> {
        self.port.read_exact(buf)?;
        debug!("{}: {buf:02X?}", cstr!("<r!>RX"));
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), io::Error> {
        debug!("{}: {buf:02X?}", cstr!("<g!>TX"));
        self.port.write_all(buf)
    }

    fn ping(&mut self) -> ResultComm<PingResponse> {
        trace!("Pinging device");
        self.write(&[0x5a, Ping::get_code()])?;

        // After power cycle, MBoot v3.0+ may respond with leading dummy data
        // We need to read data from UART until we find the frame start byte (0x5A)
        const MAX_PING_RESPONSE_DUMMY_BYTES: usize = 50;
        let mut start_byte = [0u8; 1];

        for i in 0..MAX_PING_RESPONSE_DUMMY_BYTES {
            if let Err(e) = self.port.read_exact(&mut start_byte) {
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
        self.port.read_exact(&mut frame_type)?;

        if frame_type[0] != PingResponse::get_code() {
            return Err(CommunicationError::InvalidHeader);
        }

        // Read the rest of the response (8 bytes)
        let mut response_data = [0u8; 8];
        self.port.read_exact(&mut response_data)?;

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
        self.write(&[0x5a, super::ACK])
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
            thread::sleep(polling_interval);

            if self.read_static(&mut buf).is_ok() {
                if buf[0] != 0x5a {
                    return Err(CommunicationError::InvalidHeader);
                }

                return match buf[1] {
                    ACK => Ok(()),
                    NACK => Err(CommunicationError::NACKSent),
                    ACK_ABORT => Err(CommunicationError::Aborted),
                    _ => Err(CommunicationError::InvalidHeader),
                };
            }
        }

        Err(CommunicationError::Timeout)
    }
}

#[cfg(test)]
mod tests {
    use crate::mboot::{packets::ping::PingResponse, protocols::ProtocolOpen};

    use super::UARTProtocol;

    const DEVICE: &str = "COM3";
    fn open_connection() -> UARTProtocol {
        UARTProtocol::open(DEVICE).unwrap()
    }

    #[test]
    #[ignore = "Requires hardware connection to board"]
    fn test_board_ping() {
        let mut port = open_connection();
        let expected = PingResponse {
            version: 0x00030150,
            options: 0x0000,
        };
        let res = port.ping().unwrap();
        assert_eq!(res, expected);
    }
}
