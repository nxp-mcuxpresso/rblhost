// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
pub use mboot::{
    GetPropertyResponse, KeyProvisioningResponse, McuBoot, ReadMemoryResponse, memory, packets,
    protocols::{self, CommunicationError},
    tags,
};

#[cfg(feature = "python")]
mod bindings;

#[cfg(feature = "python")]
use pyo3_stub_gen::define_stub_info_gatherer;
#[cfg(feature = "python")]
define_stub_info_gatherer!(stub_info);

mod mboot;
mod parsers;

// Only include the c_api module when the c_api feature is enabled
#[cfg(feature = "c_api")]
mod c_api;

// Re-export the C API only if the c_api feature is enabled
#[cfg(feature = "c_api")]
pub use c_api::*;
