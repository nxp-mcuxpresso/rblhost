// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! Read and parse [`PropertyTag::CurrentVersion`] tag from UART device, specified as the first CLI argument.
use mboot::{
    McuBoot,
    protocols::{ProtocolOpen, uart::UARTProtocol},
    tags::property::{PropertyTag, PropertyTagDiscriminants},
};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let device = args.next().expect("first program parameter must be a device name");

    let mut boot = McuBoot::new(UARTProtocol::open(&device)?);
    println!("sending GetProperty");
    let response = boot.get_property(PropertyTagDiscriminants::CurrentVersion, 0)?;
    if let PropertyTag::CurrentVersion(version) = response.property {
        println!(
            "received version: {version}, minor number is {}, parsed from response words {:#X?} with status code {}",
            version.minor, response.response_words, response.status
        );
    }

    Ok(())
}
