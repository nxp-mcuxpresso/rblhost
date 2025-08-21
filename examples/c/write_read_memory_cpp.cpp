/// This file works as an example, that the bindings are C++ compatible but don't directly
/// support features specific to C++.
/// The program does the same thing as write_read_memory.c
#include "../../include/mboot.h"
#include <initializer_list>
#include <iostream>
#include <string_view>

void error_check(int code, const std::string_view text) {
    if (code < 0) {
        std::cerr << "error occured: " << text << std::endl;
        exit(EXIT_FAILURE);
    }    
}

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cout << "speficy a UART device as the first argument" << std::endl;
        exit(EXIT_FAILURE);
    }

    auto device = mboot_create(argv[1], MBOOT_C_PROTOCOL_UART);
    if (device == nullptr) {
        std::cout << "device is NULL" << std::endl;
        exit(EXIT_FAILURE);
    }

    std::initializer_list<uint8_t> memory_bytes = {0x12, 0x34, 0x56};
    error_check(
        mboot_flash_erase_all(device, 0),
        "error while flash erase all"
    );
    error_check(
        mboot_write_memory(
            device,
            0,
            0, 
            memory_bytes.begin(),
            memory_bytes.size()
        ),
        "error while writing memory"
    );

    MBOOT_CReadMemoryResponse response;
    error_check(
        mboot_read_memory(
            device, 
            0, 
            static_cast<uint32_t>(memory_bytes.size()), 
            0, 
            &response
        ),
        "error while reading memory"
    );

    std::cout << std::uppercase;
    for (size_t i = 0; i < memory_bytes.size(); i++) {
        std::cout << "word " << i <<
            ": " << std::hex << "0x" << +response.bytes[i] << std::dec << std::endl;
    }

    mboot_free_read_memory_response(&response);

    mboot_destroy(device);
}

