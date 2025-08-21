// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! Erase flash, write some bytes and read them back from UART device specified as the first CLI
//! argument.
use mboot::{
    McuBoot,
    protocols::{ProtocolOpen, uart::UARTProtocol},
};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let device = args.next().expect("first program parameter must be a device name");

    // Change UARTProtocol for any other protocol you need
    let mut boot = McuBoot::new(UARTProtocol::open(&device)?);
    println!("erasing flash");
    boot.flash_erase_all(0)?;

    let memory_bytes = [0x12, 0x34, 0x56];
    println!("writing memory at address 0 with bytes: {memory_bytes:#X?}");
    boot.write_memory(0, 0, &memory_bytes)?;

    println!("reading {} bytes from memory", memory_bytes.len());
    let response = boot.read_memory(0, memory_bytes.len() as u32, 0)?;
    println!("read bytes: {:#X?}", response.bytes);
    Ok(())
}
