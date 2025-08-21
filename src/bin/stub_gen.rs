// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#[cfg(feature = "python")]
use pyo3_stub_gen::Result;

#[cfg(feature = "python")]
fn generate_python_stubs() -> Result<()> {
    let status_stubs = mboot::stub_info()?;
    status_stubs.generate()?;
    Ok(())
}

#[cfg(not(feature = "python"))]
fn main() {
    println!("Python stubs generation skipped (feature not enabled)");
}

#[cfg(feature = "python")]
fn main() -> Result<()> {
    let result = generate_python_stubs();
    println!("Python stubs generation finished.");
    result
}
