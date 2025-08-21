// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#![allow(
    clippy::wildcard_imports,
    clippy::needless_pass_by_value,
    reason = "Python is a bit less optimized"
)]

use pyo3::prelude::*;

mod mboot;
mod property;

const NOT_OPENED_ERROR: &str = "The device is not opened! Use `open()` method to open it.";

#[pymodule(name = "pymboot")]
fn mcu_boot_mod(m: &Bound<'_, PyModule>) -> PyResult<()> {
    mboot::register(m)?;
    property::register(m)?;
    Ok(())
}
