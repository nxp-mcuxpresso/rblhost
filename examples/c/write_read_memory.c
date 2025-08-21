// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
/* 
 * Erase flash, write some bytes into it and read them back from UART device,
 * specified as the first CLI argument. 
 */
#include "../../include/mboot.h"
#include <stdio.h>
#include <stdlib.h>

#define BYTE_COUNT 3

void error_check(int code, const char* text) {
    if (code < 0) {
        fprintf(stderr, "error occured: %s", text);
        exit(1);
    }
}

int main(int argc, char** argv) {
    if (argc < 2) {
        printf("specify a UART device as the first argument\n");
        exit(EXIT_FAILURE);
    }

    // Creating McuBoot device
    MBOOT_CMcuBoot *device = mboot_create(argv[1], MBOOT_C_PROTOCOL_UART);
    if (device == NULL) {
        printf("device is NULL\n");
        exit(1);
    }

    /// Memory bytes to be sent
    const uint8_t memory_bytes[BYTE_COUNT] = {0x12, 0x34, 0x56};
    // Erasing flash
    error_check(
        mboot_flash_erase_all(device, 0),
        "error while flash erase all"
    );
    // Writing memory
    error_check(
        mboot_write_memory(
            device, 
            0,
            0,
            memory_bytes,
            3
        ),
        "error while writing memory"
    );

    MBOOT_CReadMemoryResponse response;
    // Reading memory
    error_check(
        mboot_read_memory(
            device,
            0,
            3,
            0,
            &response
        ),
        "error while reading memory"
    );

    // Printing read memory
    for (size_t i = 0; i < 3; i++) {
        printf("word #%zu: 0x%X\n", i, response.bytes[i]);
    }

    // Free-ing allocated memory
    mboot_free_read_memory_response(&response);
    mboot_destroy(device);
}

