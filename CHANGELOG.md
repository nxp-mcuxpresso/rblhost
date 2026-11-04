# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), without release dates.

## [0.3.0]

### Added

- `batch COMMAND_FILE` executes commands sequentially in one RBLHOST process and device session, supports shell-style quoting and `#` comments, and stops on the first error with the source line and command.
- `efuse-read-once` and `efuse-program-once` commands, compatible with blhost (`ADDRESS DATA [nolock/lock] [--verify]`, `DATA` parsed as hex), exposed in the CLI, Rust (`McuBoot::efuse_read_once`/`efuse_program_once`, `EFUSE_LOCK_FLAG`), Python bindings and C API (`mboot_efuse_read_once`/`mboot_efuse_program_once`).
- C API (`src/c_api.rs`): extended coverage to all 17 rblhost CLI commands.
- C# bindings: new `csharp` Cargo feature generates P/Invoke bindings (`NativeMethods.g.cs`) for the full `mboot_*` C API via `csbindgen`, with a runnable example project under `examples/csharp`.
- Python wheel builds: added Python 3.15 to the `pymboot-rs` CI build matrix (`cp315-*` in `CIBW_BUILD`), bumped `pyo3` to 0.29.3 and `pyo3-stub-gen`/`pyo3-stub-gen-derive` to 0.23.1 for 3.15 compatibility, and added the `Programming Language :: Python :: 3.15` classifier.
- GitHub releases: tag builds attach prebuilt `rblhost` executables for Linux x86_64, Windows x86_64, macOS x86_64 and macOS Apple Silicon (`aarch64-apple-darwin`) with `SHA256SUMS.txt`, and the release notes are taken from the matching `CHANGELOG.md` section.

### Fixed

- `flash-image`: Intel HEX images (for example `nxpimage bootable-image export` with `output_format: hex`) were written to flash as raw text, so the device did not boot. They are now parsed like in blhost: data records are placed using the extended segment and linear address records, contiguous data is written as one segment, and record checksums are verified.
- `generate-key-blob` failed with "received another packet type than was expected", because the final response of the DEK data phase was not read before the key blob was requested.
- `flash-image`: Android SPARSE images (for example `nxpimage bootable-image export` with `output_format: sparse`) were written to flash as raw bytes, including the sparse headers, so the device did not boot. They are now expanded like in blhost: RAW and FILL chunks are written to their offsets, DONT_CARE gaps are skipped, and the CRC32 is verified.
- C API (`src/c_api.rs`): `mboot_read_memory` double-boxing and `free_response_words`/`free_bytes` using the wrong layout on free, causing corrupted data and undefined behavior.
- C API (`src/c_api.rs`): `mboot_get_status_text` and `mboot_free_status_text` were missing `#[unsafe(no_mangle)]`, so they were not exported from `libmboot`/`mboot.dll` and were silently dropped from the generated `include/mboot.h` and C# bindings, even though the README documented them as part of the public C API.
- I2C (`src/mboot/protocols/i2c/i2c_unix.rs`): a non-existing I2C slave address was reported as a misleading OS "Access denied"/"Permission denied" error instead of a clear "no response from the device" error.
