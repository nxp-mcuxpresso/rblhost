# Copyright 2025 NXP
#
# SPDX-License-Identifier: BSD-3-Clause
"""Erase flash, write some bytes and read them back from UART device specified as the first argument to the script."""
import sys
from pymboot import McuBoot

if len(sys.argv) < 2:
    print("specify a UART device as the first argument")
    exit(1)

device = sys.argv[1]

memory_bytes = [0x12, 0x34, 0x56]

with McuBoot(device) as boot:
    print("erasing flash")
    boot.flash_erase_all()
    print(f"writing memory bytes: {list(map(hex, memory_bytes))}")
    boot.write_memory(0, memory_bytes)
    print("reading memory bytes")
    response = boot.read_memory(0, len(memory_bytes))
    if response is not None:
        print(f"read memory bytes: {list(map(hex, response))}")
    else:
        print("could not read memory")
