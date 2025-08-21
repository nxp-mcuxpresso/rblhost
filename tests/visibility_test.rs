// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
use mboot::{
    CommunicationError,
    packets::{command::CommandPacket, data_phase::DataPhasePacket},
    protocols::{ProtocolOpen, i2c::I2CProtocol, uart::UARTProtocol},
    tags::{command::CommandTag, command_response::CmdResponseTag},
};

#[test]
fn test_visibility() {
    let tag = CommandTag::ReadMemory {
        start_address: 0,
        byte_count: 0xff,
        memory_id: 0,
    };
    let _ = CmdResponseTag::ReadMemory([10, 20].into());
    let _ = CommandPacket::new_data_phase(tag);
    let _ = CommunicationError::InvalidPacketReceived;
    let _ = DataPhasePacket {
        data: [10, 20, 10].to_vec(),
    };
    let _ = UARTProtocol::open("");
    let _ = I2CProtocol::open("");
}
