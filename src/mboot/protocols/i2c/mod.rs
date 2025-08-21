// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(unix)]
mod i2c_unix;

#[cfg(unix)]
pub use i2c_unix::I2CProtocol;

#[cfg(windows)]
mod i2c_windows;
#[cfg(windows)]
pub use i2c_windows::I2CProtocol;

#[allow(dead_code, reason = "not used on Windows")]
pub const DEFAULT_SLAVE: u8 = 0x10;
