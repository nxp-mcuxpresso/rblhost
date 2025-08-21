# Rblhost

<p align="center">
  <img src="img/logo.png" alt="Rblhost Logo" width="500"/>
</p>


This is a concept implementation of the blhost tool in Rust programming language. It includes `McuBoot` implementation, similar to the one in `spsdk`, together with `spsdk`-like Python bindings using `pyo3` crate.

## Features

- UART communication with configurable baudrate and timeout
- I2C communication with configurable slave address
- Command-line interface compatible with the original blhost tool
- Python bindings for integration with other tools
- C/C++ bindings

## Performance Benchmark

The Rust implementation offers performance improvements compared to both the Python SPSDK implementation and the original C/C++ blhost tool.

<p align="center">
  <img src="img/benchmark.png" alt="Performance Benchmark" width="600"/>
</p>

*Benchmark performed on MacBook Air M2 comparing rblhost with SPSDK blhost and C/C++ blhost 2.6.7. Benchmark measures get-property 1 command execution time with MCXN236 board and 57600 baud rate. Resulting time is in miliseconds.*

Key performance highlights:
- Faster startup time compared to both Python and C++ implementations
- Improved command execution speed
- Native performance with the convenience of Python bindings


## Installation

### Prerequisites

- Rust toolchain
- For Python bindings: Python 3.8+ with development headers
- libudev-dev package (for Linux)

### Building from Source

Clone the repository and cd into it:
```bash
git clone https://github.com/nxp-mcuxpresso/rblhost
cd rblhost
```

Refer to sections below to build bindings or the CLI tool.

#### Building CLI tool

Build the CLI tool:
```bash
cargo build --release
```

The binary will be available at `target/release/rblhost`. For example usage of the library, look in [examples folder](examples/).

#### Building Python bindings

1. Create and activate a virtual environment.
2. Build rblhost with Python bindings
   ```bash
   cargo build --release --features python
   ```
3. Install `pymboot` into the virtual environment
   - Normal installation
     ```bash
     pip install .
     ```
   - Development installation:
     ```bash
     pip install -e .
     ```

For examples on how to use Python bindings, look into [examples folder](examples/python).

#### Building C bindings

**Note: Compiling together `python` and `c_api` features/bindings may result in malfunctioning libraries.**

**Prerequisites:**
- Rust toolchain (rustc, cargo)
- C compiler (gcc, clang)
- For Linux: libudev-dev package
- For macOS: Xcode command line tools

1. Run the following to build C bindings:
   ```bash
   cargo build --release --features c_api
   ```
2. This build process will:
   1. Compile the Rust library
   2. Generate C header files (compatible with C++) using cbindgen
   3. Create the shared library
3. Output files are:
   - Library:
     - Linux: `target/release/libmboot.so`
     - macOS: `target/release/libmboot.dylib`
     - Windows: `target/release/mboot.dll`
   - Header file: `include/mboot.h`

For more information about the bindings, refer to [C Bindings](#mboot-c-bindings) section. For examples on how to use C bindings and an example `CMakeLists.txt` file, look into [examples folder](examples/c).

### System-specific Requirements

#### Linux
- For UART: No additional requirements
- For I2C: The i2c-dev kernel module must be loaded
  ```bash
  sudo modprobe i2c-dev
  ```

#### Windows
- For UART: No additional requirements
- I2C communication is not supported on Windows

#### macOS
- For UART: No additional requirements
- I2C communication is not supported on macOS


## CLI Usage

### Basic Command Structure

```
rblhost [OPTIONS] -- COMMAND [ARGS]...
```

### Communication Options

#### UART Connection

```
rblhost -p <port>[,<baudrate>] [OPTIONS] -- COMMAND [ARGS]...
```

- `<port>`: Serial port name (e.g., COM3, /dev/ttyUSB0)
- `<baudrate>`: Optional baudrate (default: 57600)

Example:
```
rblhost -p COM3,115200 -- reset
```

#### I2C Connection

**Basic information:**
1. Uses the Linux I2C device interface (`/dev/i2c-X`)
2. Supports specifying a slave address or uses the default (0x10)
3. Follows the same packet protocol as UART communication
4. Requires the i2c-dev kernel module to be loaded on Linux systems (refer to [System-specific Requirements](#system-specific-requirements))

```
rblhost --i2c <device>:<slave_address> [OPTIONS] -- COMMAND [ARGS]...
```

- `<device>`: I2C device path in `/dev/i2c-X` format, where `X` is the I2C bus number
- `<slave_address>`: Optional slave address in hex format (e.g., `0x3A`), default is `0x10`

Example:
```
rblhost --i2c /dev/i2c-1:0x3A -- reset
```

### Common Options

- `-t, --timeout <MILLISECONDS>`: Serial read timeout in milliseconds (default: 5000)
- `-s, --silent`: Suppress status response and response words
- `-v, --verbose`: Increase verbosity level (can be used multiple times)

Example with timeout:
```
rblhost -p COM3 -t 10000 -- flash-erase-all
```

### Available Commands

- `get-property`: Queries various bootloader properties and settings
- `reset`: Reset the device
- `execute`: Jumps to code at the provided address
- `call`: Invokes code at an address, passing an argument to it
- `flash-erase-all`: Perform an erase of the entire flash memory
- `fill-memory`: Fills the memory with a pattern
- `read-memory`: Reads the memory and writes it to a file or stdout
- `set-property`: Changes properties and options in the bootloader
- `configure-memory`: Sets a config at internal memory to memory with ID
- `flash-erase-all-unsecure`: Erase Complete Flash and Unlock
- `flash-erase-region`: Erases one or more sectors of the flash memory
- `write-memory`: Write memory from a file or CLI
- `fuse-program`: Program fuse
- `fuse-read`: Reads the fuse and writes it to the file or stdout
- `receive-sb-file`: Receives a file in a Secure Binary (SB) format
- `flash-read-once`: Read from MCU flash program once region (eFuse/OTP)
- `flash-program-once`: Write into MCU program once region (eFuse/OTP)
- `trust-provisioning`: Group of subcommands related to trust provisioning
- `key-provisioning`: Group of subcommands related to key provisioning
- `load-image`: Sends a boot image file to the device

## MBoot C Bindings

The MCU Boot library provides a C API that allows C/C++ applications to communicate with MCU bootloaders. The API provides functions for:

- Connecting to MCU bootloaders over UART
- Reading and writing memory
- Getting device properties
- Executing commands on the device

### Including the Library

1. Include the header file in your C code:
   ```c
   #include "mboot.h"
   ```

2. Compile your C program with the library:
   ```bash
   # Linux/macOS
   gcc -o my_program my_program.c -L./target/release -lmboot -Wl,-rpath,./target/release
   
   # Windows
   gcc -o my_program.exe my_program.c -L./target/release -lmboot
   ```
   You can also use cmake example in [examples](examples/c/CMakeLists.txt).

Look into the generated header to see all currently available functions.

### Error Handling

The API functions return integer status codes:

- `0`: Success
- `-1`: Invalid parameters (null pointers)
- `-2`: Invalid property tag
- `-3`: Communication error

All of these errors are specified as macros in the generated header. It's also possible to use `mbot_get_status_text` function to get a text description of the error during runtime.

### Memory Management

All functions containing Allocations section in their documentation allocate data on heap, which must be later freed.

### Thread Safety

The MCU Boot C API is not thread-safe. Do not use the same `MBOOT_CMcuBoot` instance from multiple threads simultaneously.

### Troubleshooting

#### Library Not Found

If you get a "library not found" error when running your program:

- **Linux**: Set the `LD_LIBRARY_PATH` environment variable:
  ```bash
  export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:./target/release
  ```

- **macOS**: Set the `DYLD_LIBRARY_PATH` environment variable:
  ```bash
  export DYLD_LIBRARY_PATH=$DYLD_LIBRARY_PATH:./target/release
  ```

- **Windows**: Add the library directory to your PATH or copy the DLL to the same directory as your executable.

#### Linking Errors

If you get undefined symbol errors when linking:

1. Make sure you're using the correct library name (`-lmboot`)
2. Check that the library path is correct (`-L./target/release`)
3. Verify that the library was built successfully
4. "Undefined reference to Py*" -- Build the library with only the `c_api` feature, like shown here:
   ```bash
   cargo build -rF c_api
   ```


## License

This project is licensed under the BSD 3-Clause License - see the [LICENSE](LICENSE) file for details.

## Missing and unimplemented features

- [ ] Implement all of `blhost` and `McuBoot` commands
- [ ] Add doc comments to **everything**
      This could be enforced by using [lints in rustdoc](https://doc.rust-lang.org/rustdoc/lints.html).
- [ ] Implement missing python bindings
- [ ] Address all TODO and FIXME comments
- [ ] Add different texts for different board families
- [ ] Refactor and optimize the code

# State of work

This table compares the commands available in rblhost (Rust implementation) with the original blhost (C implementation), and indicates which commands are supported in the Python bindings.

| Command                  | rblhost | blhost | Python Bindings | C Bindings |
|--------------------------|:-------:|:------:|:---------------:|:----------:|
| reset                    | ✅      | ✅     | ✅              | ❌         |
| get-property             | ✅      | ✅     | ✅              | ✅         |
| set-property             | ✅      | ✅     | ✅              | ❌         |
| flash-erase-region       | ✅      | ✅     | ✅              | ❌         |
| flash-erase-all          | ✅      | ✅     | ✅              | ✅         |
| flash-erase-all-unsecure | ✅      | ✅     | ✅              | ❌         |
| read-memory              | ✅      | ✅     | ✅              | ✅         |
| write-memory             | ✅      | ✅     | ✅              | ✅         |
| fill-memory              | ✅      | ✅     | ✅              | ❌         |
| receive-sb-file          | ✅      | ✅     | ✅              | ✅         |
| execute                  | ✅      | ✅     | ✅              | ❌         |
| call                     | ✅      | ✅     | ✅              | ❌         |
| flash-security-disable   | ❌      | ✅     | ❌              | ❌         |
| flash-program-once       | ✅      | ✅     | ✅              | ✅         |
| flash-read-once          | ✅      | ✅     | ✅              | ✅         |
| efuse-program-once       | ❌      | ✅     | ❌              | ❌         |
| efuse-read-once          | ❌      | ✅     | ❌              | ❌         |
| flash-read-resource      | ❌      | ✅     | ❌              | ❌         |
| configure-memory         | ✅      | ✅     | ✅              | ❌         |
| flash-image              | ❌      | ✅     | ❌              | ❌         |
| reliable-update          | ❌      | ✅     | ❌              | ❌         |
| generate-key-blob        | ❌      | ✅     | ❌              | ❌         |
| key-provisioning         | ✅      | ✅     | ✅              | ❌         |
| load-image               | ✅      | ✅     | ✅              | ❌         |
| program-aeskey           | ❌      | ✅     | ❌              | ❌         |
| fuse-program             | ✅      | ❌     | ✅              | ❌         |
| fuse-read                | ✅      | ❌     | ✅              | ❌         |
| trust-provisioning       | ✅      | ❌     | ✅              | ❌         |
