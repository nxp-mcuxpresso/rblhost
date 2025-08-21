// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    CommunicationError,
    mboot::ResultComm,
    protocols::{Protocol, ProtocolOpen},
};

use std::time::Duration;

#[derive(Debug)]
pub struct I2CProtocol;

impl ProtocolOpen for I2CProtocol {
    fn open(_identifier: &str) -> ResultComm<Self> {
        Err(CommunicationError::UnsupportedPlatform)
    }
}

impl Protocol for I2CProtocol {
    fn get_timeout(&self) -> Duration {
        unimplemented!("I2C not supported on Windows")
    }

    fn get_polling_interval(&self) -> Duration {
        unimplemented!("I2C not supported on Windows")
    }

    fn get_identifier(&self) -> &str {
        unimplemented!("I2C not supported on Windows")
    }

    fn read(&mut self, _bytes: usize) -> ResultComm<Vec<u8>> {
        Err(CommunicationError::UnsupportedPlatform)
    }

    fn write_packet_raw(&mut self, _data: &[u8]) -> ResultComm<()> {
        Err(CommunicationError::UnsupportedPlatform)
    }

    fn read_packet_raw(&mut self, _packet_code: u8) -> ResultComm<Vec<u8>> {
        Err(CommunicationError::UnsupportedPlatform)
    }
}
