// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! Bootloader tag definitions and utilities.
//!
//! This module contains the core tag definitions used by the MCU bootloader protocol,
//! including command tags, property tags, status codes, and response structures.
//! It also provides utilities for converting tagged enums to their numeric representations.

pub mod command;
pub mod command_flag;
pub mod command_response;
pub mod property;
pub mod status;
/// Trait for converting tagged enums to their numeric representation.
///
/// This trait provides a method to extract the numeric value from enums that use
/// `#[repr(u8)]`. It uses unsafe pointer casting to access the discriminant value directly.
///
/// # Safety
/// This trait must ONLY be used on enums with `#[repr(u8)]` attribute.
/// Using it on other types will result in undefined behavior.
///
/// # Warning
/// DO NOT use this trait on anything other than `#[repr(u8)]` enums.
pub trait ToAddress {
    /// Extract the numeric code from a tagged enum.
    ///
    /// Returns the discriminant value of the enum as a u8.
    ///
    /// # Safety
    /// This method uses unsafe pointer casting to access the enum discriminant.
    /// It is safe only when used on `#[repr(u8)]` enums.
    #[must_use]
    fn code(&self) -> u8 {
        // surprisingly this is not UB on repr types
        // do not believe me? https://doc.rust-lang.org/reference/items/enumerations.html#pointer-casting
        unsafe { *std::ptr::from_ref::<Self>(self).cast::<u8>() }
    }
}
