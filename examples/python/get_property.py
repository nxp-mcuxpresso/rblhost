# Copyright 2025 NXP
#
# SPDX-License-Identifier: BSD-3-Clause
"""Read CurrentVersion tag from UART device, specified as the first argument to the script."""
import sys
from pymboot import McuBoot, PropertyTag

if len(sys.argv) < 2:
    print("specify a UART device as the first argument")
    exit(1)

device = sys.argv[1]

with McuBoot(device) as boot:
    print("seding GetProperty")
    response = boot.get_property(PropertyTag.CurrentVersion)
    print(f"Status code: {boot.status_code}")
    print(f"Status string: {boot.status_code_str}")
    print(f"Property value: {response}")
