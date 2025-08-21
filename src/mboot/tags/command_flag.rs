// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! McuBoot Command Flag Definitions
//!
//! This module defines the command flags used in McuBoot protocol packets to indicate
//! the characteristics of commands, particularly whether they have an accompanying
//! data phase.

use super::ToAddress;

#[cfg(doc)]
use super::command::CommandTag;

/// McuBoot command flag enumeration
///
/// Represents the flags that can be set in a McuBoot command packet header to
/// indicate the command's characteristics. The primary purpose is to signal
/// whether the command will be followed by additional data packets.
///
/// # Protocol Usage
/// - Commands like [`CommandTag::GetProperty`], [`CommandTag::Reset`], [`CommandTag::Execute`] use [`CommandFlag::NoData`] flag
/// - Commands like [`CommandTag::WriteMemory`], [`CommandTag::ReceiveSBFile`] use
///   [`CommandFlag::HasDataPhase`] flag
#[repr(u8)]
#[derive(Clone, Copy, Debug, derive_more::TryFrom, derive_more::Display, strum::EnumIs)]
#[try_from(repr)]
pub enum CommandFlag {
    /// Command has no additional data following it
    #[display("Command has no data")]
    NoData = 0,

    /// Command has a data phase following it
    #[display("Command has a data phase")]
    HasDataPhase = 1,
}

impl ToAddress for CommandFlag {}
