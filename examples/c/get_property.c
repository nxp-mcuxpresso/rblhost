// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
/* Read and parse CurrentVersion tag from UART device, specified as the first CLI argument. */
#include "../../include/mboot.h"
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char** argv) {
    if (argc < 2) {
        printf("specify a UART device as the first argument\n");
        exit(EXIT_FAILURE);
    }

    // Creating McuBoot device
    MBOOT_CMcuBoot *device = mboot_create(argv[1], MBOOT_C_PROTOCOL_UART);
    if (device == NULL) {
        printf("device is NULL\n");
        exit(EXIT_FAILURE);
    }

    MBOOT_CGetPropertyResponse response;
    // Sending get_property, tag number 1 is for current version
    MBOOT_CStatus status = mboot_get_property(device, 1, 0, &response);
    
    if (status < 0) {
        fprintf(stderr, "error occured while running get_property\n");
        exit(EXIT_FAILURE);
    }

    // Reading response words
    for (size_t i = 0; i < response.response_words_len; i++) {
        fprintf(stderr, "word #%zu: 0x%X\n", i, response.response_words[i]);
    }

    // Parsing version data
    if (response.response_words_len > 0) {
        uint32_t version_word = response.response_words[0];

        char mark = (version_word >> 24) & 0xFF;
        uint8_t major = (version_word >> 16) & 0xFF;
        uint8_t minor = (version_word >> 8) & 0xFF;
        uint8_t fixation = version_word & 0xFF;

        printf("Version: %c%d.%d.%d\n", mark, major, minor, fixation);
    } else {
        fprintf(stderr, "response word length is incorrect\n");
        exit(EXIT_FAILURE);
    }
    
    // Free-ing allocated memory
    mboot_free_response_words(response.response_words);
    mboot_destroy(device);
}
