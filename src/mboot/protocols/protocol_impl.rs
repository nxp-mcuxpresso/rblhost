// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
use crate::mboot::Packet;
use crate::mboot::PacketParse;
use crate::mboot::ResultComm;
use crate::protocols::Duration;
use crate::protocols::PacketConstruct;
use enum_dispatch::enum_dispatch;

use super::{Protocol, i2c::I2CProtocol, uart::UARTProtocol, usb::USBProtocol};

/// Unified protocol implementation enum
///
/// This enum can hold any of the supported protocol implementations and
/// uses `enum_dispatch` to generate efficient dispatch code. This allows
/// the same high-level code to work with different transport protocols
/// without the overhead of dynamic dispatch via trait objects.
///
/// # Note
/// As enum dispatch might incur performance penalties, it is enabled
/// only for Python bindings, and C API code
#[enum_dispatch(Protocol)]
pub enum ProtocolImpl {
    /// UART/Serial protocol implementation
    UARTProtocol,
    /// I2C protocol implementation
    I2CProtocol,
    /// USB HID protocol implementation
    USBProtocol,
}
