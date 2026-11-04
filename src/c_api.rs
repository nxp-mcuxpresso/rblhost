// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#![warn(missing_docs)]

use crate::mboot::{
    KeyProvisioningResponse, McuBoot, ResultStatus,
    protocols::ProtocolOpen,
    tags::{
        command::{KeyProvOperation, KeyProvUserKeyType, TrustProvOperation},
        property::PropertyTagDiscriminants,
    },
};
use crate::{
    protocols::{i2c::I2CProtocol, protocol_impl::ProtocolImpl, uart::UARTProtocol, usb::USBProtocol},
    tags::status::StatusCode,
};
use std::{
    ffi::{CStr, CString},
    ptr, slice,
    str::FromStr,
};
/// [`McuBoot`] type that you can use to communicate with the device using `mboot_` functions.
///
/// This type is just an alias to `void` and in **all** instances it is a pointer to heap allocated
/// data (it may initially be `NULL`, to indicate an error). You shouldn't be needing it for data on stack.
type CMcuBoot = libc::c_void;

/// When positive indicates a [`StatusCode`]. When negative, indicates an error.
type CStatus = i32;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
/// Struct filled by [`mboot_get_property`], containing data about a property.
pub struct CGetPropertyResponse {
    /// Received status code
    pub status: CStatus,
    /// Received reponse words
    pub response_words: *mut u32,
    /// Number of `u32` elements in `response_words` (not a byte count)
    pub response_words_len: usize,
    /// Number of the property
    pub property_type: u8,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
/// Struct filled by [`mboot_read_memory`], containing data from memory read.
pub struct CReadMemoryResponse {
    /// Received status code
    pub status: CStatus,
    /// Received reponse words
    pub response_words: *mut u32,
    /// Number of `u32` elements in `response_words` (not a byte count)
    pub response_words_len: usize,
    /// Received memory bytes
    pub bytes: *mut u8,
    /// Number of bytes in `bytes`
    pub bytes_len: usize,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
/// Struct filled by [`mboot_flash_read_once`], containing data read from the OTP/eFuse region.
pub struct CFlashReadOnceResponse {
    /// Received status code
    pub status: CStatus,
    /// Raw bytes read from the OTP/eFuse region, `bytes_len` (4 or 8) bytes long, little-endian.
    pub bytes: *mut u8,
    /// Length of `bytes` in bytes
    pub bytes_len: usize,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
/// Struct filled by [`mboot_trust_provisioning`], containing the operation's response words.
pub struct CTrustProvisioningResponse {
    /// Received status code
    pub status: CStatus,
    /// Received response words
    pub response_words: *mut u32,
    /// Length of `response_words` in bytes
    pub response_words_len: usize,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
/// Struct filled by [`mboot_key_provisioning_read_key_store`], containing the key store data
/// read from the device.
pub struct CKeyStoreResponse {
    /// Received status code
    pub status: CStatus,
    /// Received response words
    pub response_words: *mut u32,
    /// Length of `response_words` in bytes
    pub response_words_len: usize,
    /// Received key store bytes
    pub bytes: *mut u8,
    /// Length of `bytes` in bytes
    pub bytes_len: usize,
}

#[repr(C)]
/// Indicates which protocol should be used when initializing.
pub enum CProtocol {
    /// Use UART protocol
    UART,
    /// Use I2C protocol
    I2c,
    /// Use USB-HID protocol
    Usb,
}

/// One of the passed pointers in function arguments was NULL.
pub const ERROR_NULL_POINTER_ARG: CStatus = -1;
/// Invalid property tag passed.
pub const ERROR_INVALID_PROPERTY_TAG: CStatus = -2;
/// Error occured while communication with the device.
pub const ERROR_COMMUNICATION_ERROR: CStatus = -3;
/// Invalid key provisioning user key type passed.
pub const ERROR_INVALID_KEY_TYPE: CStatus = -4;

/// Get a mutable reference to [`McuBoot`] from mutable raw pointer.
///
/// # Safety
/// `mboot` must be a valid non-freed pointer.
unsafe fn get_mboot<'a>(mboot: *mut CMcuBoot) -> &'a mut McuBoot<ProtocolImpl> {
    unsafe { &mut *mboot.cast::<McuBoot<ProtocolImpl>>() }
}

#[unsafe(no_mangle)]
/// Get text description of the passed status code.
///
/// # Allocations
/// The status text is allocated on heap, to free it call [`mboot_free_status_text`]
/// function.
///
/// # Safety
/// This function allocates the returned `char` pointer.
#[expect(
    clippy::missing_panics_doc,
    reason = "CString panics only if the data contains 0, this will not happen with the status string"
)]
#[must_use]
pub unsafe extern "C" fn mboot_get_status_text(status: CStatus) -> *mut libc::c_char {
    let text: CString = match u32::try_from(status) {
        Ok(status) => match StatusCode::try_from(status) {
            Ok(status) => CString::from_str(&status.to_string()).unwrap(),
            Err(_) => CString::from_str("invalid status code").unwrap(),
        },
        Err(_) => CString::from_str(match status {
            ERROR_NULL_POINTER_ARG => "passed NULL pointer in function argument",
            ERROR_INVALID_PROPERTY_TAG => "invalid propery tag passed in arguments",
            ERROR_COMMUNICATION_ERROR => "error while communicating with the device",
            ERROR_INVALID_KEY_TYPE => "invalid key provisioning user key type passed in arguments",
            _ => "unknown status code",
        })
        .unwrap(),
    };
    text.into_raw()
}

#[unsafe(no_mangle)]
/// Free text description of a status code.
///
/// # Safety
///
/// UB occurs if `status_text` is an invalid pointer or has been already freed.
pub unsafe extern "C" fn mboot_free_status_text(status_text: *mut libc::c_char) {
    if !status_text.is_null() {
        drop(unsafe { CString::from_raw(status_text) });
    }
}

/// Convert `ResultStatus` to `CStatus`.
fn return_error(status: &ResultStatus) -> CStatus {
    match status {
        Ok(status) => *status as CStatus,
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

/// Free any single value allocated with [`Box`] (not a slice - see [`free_boxed_slice`] for
/// `Box<[T]>` data).
///
/// # Safety
/// `value` must be a valid non-freed pointer, originally obtained from `Box::into_raw` on a
/// `Box<T>` (not `Box<[T]>`).
unsafe fn free_box_data<T>(value: *mut T) {
    if !value.is_null() {
        drop(unsafe { Box::from_raw(value) });
    }
}

/// Free a `Box<[T]>` slice allocated with [`Box::into_raw`] and later `cast`-ed to a thin
/// pointer (as done for `response_words`/`bytes` fields across this module).
///
/// `Box<[T]>` is a fat pointer carrying both the data pointer and the element count; once cast
/// to `*mut T` for the C ABI, the count is lost and must be supplied by the caller (`len`) to
/// reconstruct a correctly-laid-out `Box<[T]>` for deallocation. Calling [`free_box_data`]
/// (which reconstructs `Box<T>`, a single element) on such a pointer deallocates with the wrong
/// size/layout and is undefined behavior for any `len != 1`.
///
/// # Safety
/// `ptr` must be either null or a valid, non-freed pointer previously obtained by casting
/// `Box::into_raw(data)` to `*mut T`, where `data: Box<[T]>` had exactly `len` elements.
unsafe fn free_boxed_slice<T>(ptr: *mut T, len: usize) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

#[unsafe(no_mangle)]
/// Create a new [`CMcuBoot`] instance from a device path.
///
/// Returns either a valid [`CMcuBoot`] instance or a NULL pointer, if any errors occur.
///
/// `device_path` format depends on `protocol`:
/// - [`CProtocol::UART`]: serial port name, optionally followed by `,<baudrate>`
///   (e.g. `"COM1,38400"` or `"/dev/ttyACM0"`). Default baudrate is used if omitted.
/// - [`CProtocol::I2c`]: I2C device identifier, e.g. `"/dev/i2c-1:0x10"`.
/// - [`CProtocol::Usb`]: same identifier format accepted by the `-u/--usb` CLI option,
///   e.g. `"0x1FC9,0x0135"` (`<vid>[,<pid>]`, decimal or `0x`-prefixed hex). If several
///   identical boards (same VID/PID) are connected at once, a third field can be added
///   to target one specific device by OS path/serial/USB-topology-path selector, e.g.
///   `"0x1FC9,0x0135,6&7909BF&0&0000"` or, by USB topology path on Linux/macOS,
///   `"0x1FC9,0x0135,1-1.4.3"`.
///
/// # Allocations
/// A valid [`CMcuBoot`] instance must be freed when not used with [`mboot_destroy`] function.
///
/// # Safety
///
/// If `device_path` is non-null, it must point to a valid, null-terminated UTF-8 C string.
/// Undefined behavior may occur if the pointer is invalid or the string is not properly terminated.
/// If this function returns a valid [`CMcuBoot`] instance, it must be later freed.
pub unsafe extern "C" fn mboot_create(device_path: *const libc::c_char, protocol: CProtocol) -> *mut CMcuBoot {
    let c_str = unsafe { CStr::from_ptr(device_path) };
    let Ok(device_path_str) = c_str.to_str() else {
        return ptr::null_mut();
    };

    let device: ProtocolImpl = match protocol {
        CProtocol::UART => match UARTProtocol::open(device_path_str) {
            Ok(p) => p.into(),
            Err(_) => return ptr::null_mut(),
        },
        CProtocol::I2c => match I2CProtocol::open(device_path_str) {
            Ok(p) => p.into(),
            Err(_) => return ptr::null_mut(),
        },
        CProtocol::Usb => match USBProtocol::open(device_path_str) {
            Ok(p) => p.into(),
            Err(_) => return ptr::null_mut(),
        },
    };

    let mboot = Box::new(McuBoot::new(device));
    Box::into_raw(mboot).cast::<CMcuBoot>()
}

#[unsafe(no_mangle)]
/// Destroys a [`CMcuBoot`] instance and frees its resources.
///
/// # Safety
/// If `mboot` is non-null, it must be a valid pointer returned by [`mboot_create`].
/// Passing an invalid or already-freed pointer results in undefined behavior.
pub unsafe extern "C" fn mboot_destroy(mboot: *mut CMcuBoot) {
    unsafe { free_box_data(mboot.cast::<McuBoot<ProtocolImpl>>()) };
}

#[unsafe(no_mangle)]
/// Retrieves a bootloader property and writes the result to the response struct.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Allocations
/// This function allocates an array in `response_words` field in `response` parameter. Use
/// [`mboot_free_response_words`] (passing `response.response_words_len` as `len`) to free it.
///
/// # Safety
/// `mboot` and `response` should be non-null and they must be valid pointers.
/// `response` must point to writable memory. Passing invalid pointers results in UB.
pub unsafe extern "C" fn mboot_get_property(
    mboot: *mut CMcuBoot,
    tag: u8,
    memory_index: u32,
    response: *mut CGetPropertyResponse,
) -> CStatus {
    if mboot.is_null() || response.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let response = unsafe { &mut *response };

    *response = CGetPropertyResponse::default();

    let mboot = unsafe { get_mboot(mboot) };
    let Ok(tag_enum) = PropertyTagDiscriminants::try_from(tag) else {
        return ERROR_INVALID_PROPERTY_TAG;
    };

    match mboot.get_property(tag_enum, memory_index) {
        Ok(res) => {
            // Create a copy of the response words
            let words: Box<[u32]> = if res.response_words.is_empty() {
                // If empty, return a single zero word
                Box::new([0u32])
            } else {
                // Clone the existing words
                res.response_words
            };

            let words_len = words.len();
            let words_ptr = Box::into_raw(words);
            let status = res.status as CStatus;

            *response = CGetPropertyResponse {
                status,
                response_words: words_ptr.cast::<u32>(),
                response_words_len: words_len,
                property_type: tag,
            };

            status
        }
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

#[unsafe(no_mangle)]
/// Reads memory from the device and writes the result to the response struct.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Allocations
/// This function allocates arrays in `response_words` and `bytes` fields in `response` parameter.
/// To free them both use [`mboot_free_read_memory_response`] function. It's also possible to call
/// [`mboot_free_response_words`] on `response_words` field (passing `response.response_words_len`
/// as `len`) **and** [`mboot_free_bytes`] on `bytes` field (passing `response.bytes_len` as
/// `len`) to free them in any order you need.
///
/// # Safety
/// `mboot` and `response` should be non-null and they must be valid pointers.
/// `response` must point to writable memory. Invalid or misaligned pointers cause undefined behavior.
pub unsafe extern "C" fn mboot_read_memory(
    mboot: *mut CMcuBoot,
    start_address: u32,
    byte_count: u32,
    memory_id: u32,
    response: *mut CReadMemoryResponse,
) -> libc::c_int {
    if mboot.is_null() || response.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    // Initialize the response struct with zeros
    let response = unsafe { &mut *response };
    *response = CReadMemoryResponse::default();

    let mboot = unsafe { get_mboot(mboot) };

    match mboot.read_memory(start_address, byte_count, memory_id) {
        Ok(res) => {
            // Create a copy of the response words
            let words: Box<[u32]> = if res.response_words.is_empty() {
                // If empty, return a single zero word
                Box::new([0u32])
            } else {
                // Clone the existing words
                res.response_words
            };
            let words_len = words.len();
            let words_ptr = Box::into_raw(words);

            // Add explicit type annotation for bytes
            let bytes: Box<[u8]> = if res.bytes.is_empty() {
                // If empty, return a single zero byte
                Box::new([0u8])
            } else {
                // Clone the existing bytes
                res.bytes
            };

            let bytes_len = bytes.len();
            let bytes_ptr = Box::into_raw(bytes).cast::<u8>();
            let status = res.status as CStatus;

            *response = CReadMemoryResponse {
                status,
                response_words: words_ptr.cast::<u32>(),
                response_words_len: words_len,
                bytes: bytes_ptr,
                bytes_len,
            };

            status
        }
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

#[unsafe(no_mangle)]
/// Writes memory from the device and returns status code.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `byte_count` must be lower or the same as the number of bytes in `bytes` array. `mboot` and
/// `bytes`, should be non-null and must be valid pointers.
pub unsafe extern "C" fn mboot_write_memory(
    mboot: *mut CMcuBoot,
    start_address: u32,
    memory_id: u32,
    bytes: *const u8,
    byte_count: usize,
) -> CStatus {
    if mboot.is_null() || bytes.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let mboot = unsafe { get_mboot(mboot) };
    let bytes = unsafe { slice::from_raw_parts(bytes, byte_count) };

    return_error(&mboot.write_memory(start_address, memory_id, bytes))
}

#[unsafe(no_mangle)]
/// Perform an erase of the entire flash memory, excluding protected regions.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_flash_erase_all(mboot: *mut CMcuBoot, memory_id: u32) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.flash_erase_all(memory_id))
}

#[unsafe(no_mangle)]
/// Run `receive_sb_file` command on the device.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `byte_count` must be lower or the same as the number of bytes in `bytes` array. `mboot` and
/// `bytes`, should be non-null and must be valid pointers.
pub unsafe extern "C" fn mboot_receive_sb_file(mboot: *mut CMcuBoot, bytes: *const u8, byte_count: usize) -> CStatus {
    if mboot.is_null() || bytes.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let bytes = unsafe { slice::from_raw_parts(bytes, byte_count) };
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.receive_sb_file(bytes))
}

#[unsafe(no_mangle)]
/// Write into program once region (eFuse/OTP) on device.
///
/// `count` must be 4 or 8 (matching blhost); any other value returns
/// [`ERROR_COMMUNICATION_ERROR`]. Only the low `count` bytes of `data` are sent on the wire.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_flash_program_once(
    mboot: *mut CMcuBoot,
    index: u32,
    count: u32,
    data: u64,
    verify: bool,
) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.flash_program_once(index, count, data, verify))
}

#[unsafe(no_mangle)]
/// Read from program once region (eFuse/OTP) on device.
///
/// `count` must be 4 or 8 (matching blhost); any other value returns
/// [`ERROR_COMMUNICATION_ERROR`].
///
/// Returns a positive integer with a status code on success or a negative integer on error.
/// The data read (`count` bytes) is written to `response`.
///
/// # Allocations
/// This function allocates an array in the `bytes` field in `response` parameter. Use
/// [`mboot_free_flash_read_once_response`] to free it.
///
/// # Safety
/// `mboot` and `response` should be non-null and they must be valid pointers.
/// `response` must point to writable memory. Passing invalid pointers results in UB.
pub unsafe extern "C" fn mboot_flash_read_once(
    mboot: *mut CMcuBoot,
    index: u32,
    count: u32,
    response: *mut CFlashReadOnceResponse,
) -> CStatus {
    if mboot.is_null() || response.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let response = unsafe { &mut *response };
    *response = CFlashReadOnceResponse::default();

    let mboot = unsafe { get_mboot(mboot) };
    match mboot.flash_read_once(index, count) {
        Ok(data) => {
            let status = StatusCode::Success as CStatus;
            let bytes_len = data.len();
            // An OTP read always returns `count` (4 or 8) bytes on success; `bytes_len == 0`
            // only indicates no data was allocated (unlike `mboot_read_memory`, there is no
            // valid empty-but-successful read here, so we don't fabricate a fake 1-byte buffer).
            let bytes_ptr = if data.is_empty() {
                ptr::null_mut()
            } else {
                Box::into_raw(data).cast::<u8>()
            };

            *response = CFlashReadOnceResponse {
                status,
                bytes: bytes_ptr,
                bytes_len,
            };

            status
        }
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

#[unsafe(no_mangle)]
/// Write a 32-bit word into the eFuse/OTP region on device.
///
/// Equivalent to [`mboot_flash_program_once`] with `count` of 4, matching blhost's
/// `efuse-program-once`. Set bit 24 of `index` to lock the word.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_efuse_program_once(
    mboot: *mut CMcuBoot,
    index: u32,
    data: u32,
    verify: bool,
) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.efuse_program_once(index, data, verify))
}

#[unsafe(no_mangle)]
/// Read a 32-bit word from the eFuse/OTP region on device.
///
/// Equivalent to [`mboot_flash_read_once`] with `count` of 4, matching blhost's
/// `efuse-read-once`. The read value is written to `value`.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` and `value` should be non-null and they must be valid pointers.
/// `value` must point to writable memory. Passing invalid pointers results in UB.
pub unsafe extern "C" fn mboot_efuse_read_once(mboot: *mut CMcuBoot, index: u32, value: *mut u32) -> CStatus {
    if mboot.is_null() || value.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    match mboot.efuse_read_once(index) {
        Ok(read) => {
            unsafe { *value = read };
            StatusCode::Success as CStatus
        }
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

#[unsafe(no_mangle)]
/// Reset the device.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
/// The connection to the device may be lost after a successful reset.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_reset(mboot: *mut CMcuBoot) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.reset())
}

#[unsafe(no_mangle)]
/// Changes a property in the bootloader. Accepts the same property tag used by
/// [`mboot_get_property`].
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_set_property(mboot: *mut CMcuBoot, tag: u8, value: u32) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let Ok(tag_enum) = PropertyTagDiscriminants::try_from(tag) else {
        return ERROR_INVALID_PROPERTY_TAG;
    };
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.set_property(tag_enum, value))
}

#[unsafe(no_mangle)]
/// Erases one or more sectors of flash memory.
///
/// `start_address` and `byte_count` must be a multiple of the word size. The entire
/// sector(s) containing the start and end address are erased.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_flash_erase_region(
    mboot: *mut CMcuBoot,
    start_address: u32,
    byte_count: u32,
    memory_id: u32,
) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.flash_erase_region(start_address, byte_count, memory_id))
}

#[unsafe(no_mangle)]
/// Erases the complete flash memory and recovers the flash security section, effectively
/// unsecuring the device.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_flash_erase_all_unsecure(mboot: *mut CMcuBoot) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.flash_erase_all_unsecure())
}

#[unsafe(no_mangle)]
/// Fills a memory region with a 32-bit pattern.
///
/// `start_address` and `byte_count` must be word aligned.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_fill_memory(
    mboot: *mut CMcuBoot,
    start_address: u32,
    byte_count: u32,
    pattern: u32,
) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.fill_memory(start_address, byte_count, pattern))
}

#[unsafe(no_mangle)]
/// Jumps to code at the provided address. The system is returned to a reset state before the
/// jump.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_execute(
    mboot: *mut CMcuBoot,
    start_address: u32,
    argument: u32,
    stackpointer: u32,
) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.execute(start_address, argument, stackpointer))
}

#[unsafe(no_mangle)]
/// Invokes code at an address, passing an argument to it.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_call(mboot: *mut CMcuBoot, start_address: u32, argument: u32) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.call(start_address, argument))
}

#[unsafe(no_mangle)]
/// Sets a config at internal memory to memory with ID. The specified configuration block
/// must have been previously written to memory using [`mboot_write_memory`].
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_configure_memory(mboot: *mut CMcuBoot, memory_id: u32, address: u32) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.configure_memory(memory_id, address))
}

#[unsafe(no_mangle)]
/// Sends a boot image's raw bytes directly to the device, without a command header.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `byte_count` must be lower or the same as the number of bytes in `bytes` array. `mboot` and
/// `bytes`, should be non-null and must be valid pointers.
pub unsafe extern "C" fn mboot_load_image(mboot: *mut CMcuBoot, bytes: *const u8, byte_count: usize) -> CStatus {
    if mboot.is_null() || bytes.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let bytes = unsafe { slice::from_raw_parts(bytes, byte_count) };
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.load_image(bytes))
}

#[unsafe(no_mangle)]
/// Writes data to the device's fuse memory region. This operation is permanent and
/// irreversible.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `byte_count` must be lower or the same as the number of bytes in `bytes` array. `mboot` and
/// `bytes`, should be non-null and must be valid pointers.
pub unsafe extern "C" fn mboot_fuse_program(
    mboot: *mut CMcuBoot,
    start_address: u32,
    memory_id: u32,
    bytes: *const u8,
    byte_count: usize,
) -> CStatus {
    if mboot.is_null() || bytes.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let bytes = unsafe { slice::from_raw_parts(bytes, byte_count) };
    let mboot = unsafe { get_mboot(mboot) };
    return_error(&mboot.fuse_program(start_address, memory_id, bytes))
}

#[unsafe(no_mangle)]
/// Reads data from the device's fuse memory region and writes the result to the response
/// struct.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Allocations
/// This function allocates arrays in `response_words` and `bytes` fields in `response` parameter.
/// Use [`mboot_free_read_memory_response`] to free them both.
///
/// # Safety
/// `mboot` and `response` should be non-null and they must be valid pointers.
/// `response` must point to writable memory. Invalid or misaligned pointers cause undefined behavior.
pub unsafe extern "C" fn mboot_fuse_read(
    mboot: *mut CMcuBoot,
    start_address: u32,
    byte_count: u32,
    memory_id: u32,
    response: *mut CReadMemoryResponse,
) -> CStatus {
    if mboot.is_null() || response.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let response = unsafe { &mut *response };
    *response = CReadMemoryResponse::default();

    let mboot = unsafe { get_mboot(mboot) };

    match mboot.fuse_read(start_address, byte_count, memory_id) {
        Ok(res) => {
            let words: Box<[u32]> = if res.response_words.is_empty() {
                Box::new([0u32])
            } else {
                res.response_words
            };
            let words_len = words.len();
            let words_ptr = Box::into_raw(words);

            let bytes: Box<[u8]> = if res.bytes.is_empty() {
                Box::new([0u8])
            } else {
                res.bytes
            };

            let bytes_len = bytes.len();
            let bytes_ptr = Box::into_raw(bytes).cast::<u8>();
            let status = res.status as CStatus;

            *response = CReadMemoryResponse {
                status,
                response_words: words_ptr.cast::<u32>(),
                response_words_len: words_len,
                bytes: bytes_ptr,
                bytes_len,
            };

            status
        }
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

#[unsafe(no_mangle)]
/// Generates an OEM master share for initial trust provisioning and writes the resulting
/// response words to `response`.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Allocations
/// This function allocates an array in the `response_words` field of `response`. Use
/// [`mboot_free_trust_provisioning_response`] to free it.
///
/// # Safety
/// `mboot` and `response` should be non-null and they must be valid pointers.
/// `response` must point to writable memory. Passing invalid pointers results in UB.
pub unsafe extern "C" fn mboot_trust_provisioning_oem_gen_master_share(
    mboot: *mut CMcuBoot,
    oem_share_input_addr: u32,
    oem_share_input_size: u32,
    oem_enc_share_output_addr: u32,
    oem_enc_share_output_size: u32,
    oem_enc_master_share_output_addr: u32,
    oem_enc_master_share_output_size: u32,
    oem_cust_cert_puk_output_addr: u32,
    oem_cust_cert_puk_output_size: u32,
    response: *mut CTrustProvisioningResponse,
) -> CStatus {
    if mboot.is_null() || response.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let operation = TrustProvOperation::OemGenMasterShare {
        oem_share_input_addr,
        oem_share_input_size,
        oem_enc_share_output_addr,
        oem_enc_share_output_size,
        oem_enc_master_share_output_addr,
        oem_enc_master_share_output_size,
        oem_cust_cert_puk_output_addr,
        oem_cust_cert_puk_output_size,
    };

    unsafe { mboot_trust_provisioning_run(mboot, &operation, response) }
}

#[unsafe(no_mangle)]
/// Sets the OEM master share to complete trust provisioning and writes the resulting response
/// words to `response`.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Allocations
/// This function allocates an array in the `response_words` field of `response`. Use
/// [`mboot_free_trust_provisioning_response`] to free it.
///
/// # Safety
/// `mboot` and `response` should be non-null and they must be valid pointers.
/// `response` must point to writable memory. Passing invalid pointers results in UB.
pub unsafe extern "C" fn mboot_trust_provisioning_oem_set_master_share(
    mboot: *mut CMcuBoot,
    oem_share_input_addr: u32,
    oem_share_input_size: u32,
    oem_enc_master_share_input_addr: u32,
    oem_enc_master_share_input_size: u32,
    response: *mut CTrustProvisioningResponse,
) -> CStatus {
    if mboot.is_null() || response.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let operation = TrustProvOperation::OemSetMasterShare {
        oem_share_input_addr,
        oem_share_input_size,
        oem_enc_master_share_input_addr,
        oem_enc_master_share_input_size,
    };

    unsafe { mboot_trust_provisioning_run(mboot, &operation, response) }
}

/// Shared implementation for `mboot_trust_provisioning_oem_*` functions.
///
/// # Safety
/// `mboot` and `response` must be non-null, valid pointers. `response` must point to writable
/// memory.
unsafe fn mboot_trust_provisioning_run(
    mboot: *mut CMcuBoot,
    operation: &TrustProvOperation,
    response: *mut CTrustProvisioningResponse,
) -> CStatus {
    let response = unsafe { &mut *response };
    *response = CTrustProvisioningResponse::default();

    let mboot = unsafe { get_mboot(mboot) };

    match mboot.trust_provisioning(operation) {
        Ok((status, data)) => {
            let words: Box<[u32]> = if data.is_empty() { Box::new([0u32]) } else { data };
            let words_len = words.len();
            let words_ptr = Box::into_raw(words);
            let status = status as CStatus;

            *response = CTrustProvisioningResponse {
                status,
                response_words: words_ptr.cast::<u32>(),
                response_words_len: words_len,
            };

            status
        }
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

#[unsafe(no_mangle)]
/// Enrolls the key provisioning feature on the device. No additional arguments are required.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_key_provisioning_enroll(mboot: *mut CMcuBoot) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    unsafe { mboot_key_provisioning_run_status(mboot, &KeyProvOperation::Enroll) }
}

#[unsafe(no_mangle)]
/// Sends the plaintext user key specified by `key_type` to the bootloader.
///
/// See [`crate::tags::command::KeyProvUserKeyType`] for valid `key_type` values.
///
/// Returns a positive integer with a status code on success, [`ERROR_INVALID_KEY_TYPE`] if
/// `key_type` is not a recognized key type, or another negative integer on error.
///
/// # Safety
/// `key_count` must be lower or the same as the number of bytes in `key_data` array. `mboot` and
/// `key_data`, should be non-null and must be valid pointers.
pub unsafe extern "C" fn mboot_key_provisioning_set_user_key(
    mboot: *mut CMcuBoot,
    key_type: u32,
    key_data: *const u8,
    key_count: usize,
) -> CStatus {
    if mboot.is_null() || key_data.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let Ok(key_type) = KeyProvUserKeyType::try_from(key_type) else {
        return ERROR_INVALID_KEY_TYPE;
    };
    let key_data: Box<[u8]> = unsafe { slice::from_raw_parts(key_data, key_count) }.into();
    let operation = KeyProvOperation::SetUserKey { key_type, key_data };
    unsafe { mboot_key_provisioning_run_status(mboot, &operation) }
}

#[unsafe(no_mangle)]
/// Generates an intrinsic key of specified `key_type` and `key_size` on the device.
///
/// See [`crate::tags::command::KeyProvUserKeyType`] for valid `key_type` values.
///
/// Returns a positive integer with a status code on success, [`ERROR_INVALID_KEY_TYPE`] if
/// `key_type` is not a recognized key type, or another negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_key_provisioning_set_key(mboot: *mut CMcuBoot, key_type: u32, key_size: u32) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let Ok(key_type) = KeyProvUserKeyType::try_from(key_type) else {
        return ERROR_INVALID_KEY_TYPE;
    };
    let operation = KeyProvOperation::SetKey { key_type, key_size };
    unsafe { mboot_key_provisioning_run_status(mboot, &operation) }
}

#[unsafe(no_mangle)]
/// Writes the previously set key to non-volatile storage identified by `memory_id`.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_key_provisioning_write_key_nonvolatile(mboot: *mut CMcuBoot, memory_id: u32) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    unsafe { mboot_key_provisioning_run_status(mboot, &KeyProvOperation::WriteKeyNonvolatile { memory_id }) }
}

#[unsafe(no_mangle)]
/// Loads the key from non-volatile memory identified by `memory_id` into the bootloader.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_key_provisioning_read_key_nonvolatile(mboot: *mut CMcuBoot, memory_id: u32) -> CStatus {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    unsafe { mboot_key_provisioning_run_status(mboot, &KeyProvOperation::ReadKeyNonvolatile { memory_id }) }
}

#[unsafe(no_mangle)]
/// Writes key store data to the bootloader.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `keystore_count` must be lower or the same as the number of bytes in `keystore_data` array.
/// `mboot` and `keystore_data`, should be non-null and must be valid pointers.
pub unsafe extern "C" fn mboot_key_provisioning_write_key_store(
    mboot: *mut CMcuBoot,
    keystore_data: *const u8,
    keystore_count: usize,
) -> CStatus {
    if mboot.is_null() || keystore_data.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }
    let keystore_data: Box<[u8]> = unsafe { slice::from_raw_parts(keystore_data, keystore_count) }.into();
    let operation = KeyProvOperation::WriteKeyStore { keystore_data };
    unsafe { mboot_key_provisioning_run_status(mboot, &operation) }
}

#[unsafe(no_mangle)]
/// Reads the key store from the bootloader to the host and writes the result to the response
/// struct.
///
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Allocations
/// This function allocates arrays in `response_words` and `bytes` fields in `response`
/// parameter. Use [`mboot_free_key_store_response`] to free them both.
///
/// # Safety
/// `mboot` and `response` should be non-null and they must be valid pointers.
/// `response` must point to writable memory. Invalid or misaligned pointers cause undefined
/// behavior.
pub unsafe extern "C" fn mboot_key_provisioning_read_key_store(
    mboot: *mut CMcuBoot,
    response: *mut CKeyStoreResponse,
) -> CStatus {
    if mboot.is_null() || response.is_null() {
        return ERROR_NULL_POINTER_ARG;
    }

    let response = unsafe { &mut *response };
    *response = CKeyStoreResponse::default();

    let mboot = unsafe { get_mboot(mboot) };
    // file/use_hexdump are only used by the CLI for display purposes; the operation code sent
    // to the device does not depend on them, so any placeholder values are fine here.
    let operation = KeyProvOperation::ReadKeyStore {
        file: String::new(),
        use_hexdump: false,
    };

    match mboot.key_provisioning(&operation) {
        Ok(KeyProvisioningResponse::KeyStore {
            status,
            response_words,
            bytes,
        }) => {
            let words: Box<[u32]> = if response_words.is_empty() {
                Box::new([0u32])
            } else {
                response_words
            };
            let words_len = words.len();
            let words_ptr = Box::into_raw(words);

            let bytes: Box<[u8]> = if bytes.is_empty() { Box::new([0u8]) } else { bytes };
            let bytes_len = bytes.len();
            let bytes_ptr = Box::into_raw(bytes).cast::<u8>();
            let status = status as CStatus;

            *response = CKeyStoreResponse {
                status,
                response_words: words_ptr.cast::<u32>(),
                response_words_len: words_len,
                bytes: bytes_ptr,
                bytes_len,
            };

            status
        }
        Ok(KeyProvisioningResponse::Status(_)) | Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

/// Shared implementation for `mboot_key_provisioning_*` functions that only return a status
/// code (i.e. everything except [`mboot_key_provisioning_read_key_store`]).
///
/// # Safety
/// `mboot` must be a non-null, valid pointer.
unsafe fn mboot_key_provisioning_run_status(mboot: *mut CMcuBoot, operation: &KeyProvOperation) -> CStatus {
    let mboot = unsafe { get_mboot(mboot) };
    match mboot.key_provisioning(operation) {
        Ok(KeyProvisioningResponse::Status(status) | KeyProvisioningResponse::KeyStore { status, .. }) => {
            status as CStatus
        }
        Err(_) => ERROR_COMMUNICATION_ERROR,
    }
}

#[unsafe(no_mangle)]
/// Free memory allocated for response words returned by a previous call.
///
/// `len` must be the corresponding `response_words_len` value from the response struct that
/// `words` was obtained from (i.e. the number of `u32` elements, matching the original
/// allocation - **not** `response_words_len` interpreted as a byte count).
///
/// # Safety
///
/// `words` should be non-null and must be a valid pointer returned by this API. `len` must
/// match the length used to allocate `words` (see `response_words_len` on the originating
/// response struct). Passing an invalid or already-freed pointer, or a `len` that does not
/// match the original allocation, results in undefined behavior.
pub unsafe extern "C" fn mboot_free_response_words(words: *mut u32, len: usize) {
    unsafe { free_boxed_slice(words, len) }
}

#[unsafe(no_mangle)]
/// Free memory allocated for a byte buffer returned by a previous call.
///
/// `len` must be the corresponding `bytes_len` value from the response struct that `bytes` was
/// obtained from, matching the original allocation.
///
/// # Safety
///
/// `bytes` should be non-null and must be a valid pointer returned by this API. `len` must
/// match the length used to allocate `bytes` (see `bytes_len` on the originating response
/// struct). Passing an invalid or already-freed pointer, or a `len` that does not match the
/// original allocation, results in undefined behavior.
pub unsafe extern "C" fn mboot_free_bytes(bytes: *mut u8, len: usize) {
    unsafe { free_boxed_slice(bytes, len) }
}

#[unsafe(no_mangle)]
/// Free `response_words` and `bytes` in `response`.
///
/// # Safety
/// UB occurs if any data in `response` have already been freed.
pub unsafe extern "C" fn mboot_free_read_memory_response(response: *mut CReadMemoryResponse) {
    if response.is_null() {
        return;
    }
    let response = unsafe { *response };
    unsafe {
        free_boxed_slice(response.response_words, response.response_words_len);
        free_boxed_slice(response.bytes, response.bytes_len);
    }
}

#[unsafe(no_mangle)]
/// Free `bytes` in `response`.
///
/// # Safety
/// UB occurs if `bytes` in `response` has already been freed.
pub unsafe extern "C" fn mboot_free_flash_read_once_response(response: *mut CFlashReadOnceResponse) {
    if response.is_null() {
        return;
    }
    let response = unsafe { *response };
    if !response.bytes.is_null() {
        // `response.bytes` was allocated as `Box<[u8]>` (a slice, with `bytes_len` as its
        // length), not `Box<u8>`. Reconstructing it as a slice box (instead of going through
        // `free_box_data::<u8>`, which would call `Box::from_raw` on a single `u8` and
        // deallocate with the wrong size/layout) is required to avoid undefined behavior.
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(response.bytes, response.bytes_len)) });
    }
}

#[unsafe(no_mangle)]
/// Free `response_words` in `response`.
///
/// # Safety
/// UB occurs if `response_words` in `response` has already been freed.
pub unsafe extern "C" fn mboot_free_trust_provisioning_response(response: *mut CTrustProvisioningResponse) {
    if response.is_null() {
        return;
    }
    let response = unsafe { *response };
    if !response.response_words.is_null() {
        // `response_words` was allocated as `Box<[u32]>` (a slice, with `response_words_len` as
        // its length), not `Box<u32>`. Reconstructing it as a slice box (instead of going
        // through `free_box_data::<u32>`, which would call `Box::from_raw` on a single `u32` and
        // deallocate with the wrong size/layout) is required to avoid undefined behavior.
        drop(unsafe {
            Box::from_raw(ptr::slice_from_raw_parts_mut(
                response.response_words,
                response.response_words_len,
            ))
        });
    }
}

#[unsafe(no_mangle)]
/// Free `response_words` and `bytes` in `response`.
///
/// # Safety
/// UB occurs if any data in `response` have already been freed.
pub unsafe extern "C" fn mboot_free_key_store_response(response: *mut CKeyStoreResponse) {
    if response.is_null() {
        return;
    }
    let response = unsafe { *response };
    if !response.response_words.is_null() {
        // See the comment in `mboot_free_trust_provisioning_response` for why this must be
        // reconstructed as a boxed slice rather than via `free_box_data`.
        drop(unsafe {
            Box::from_raw(ptr::slice_from_raw_parts_mut(
                response.response_words,
                response.response_words_len,
            ))
        });
    }
    if !response.bytes.is_null() {
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(response.bytes, response.bytes_len)) });
    }
}

#[cfg(test)]
mod tests {
    use super::{free_box_data, free_boxed_slice};
    use std::slice;

    /// Regression test for the `mboot_read_memory` double-boxing bug (bug 1):
    /// allocating a `Box<[u32]>`, converting it to a raw thin pointer the same way
    /// `mboot_read_memory`/`mboot_get_property` do, and reading it back via
    /// `slice::from_raw_parts` must yield the original data - not fat-pointer metadata
    /// bytes reinterpreted as `u32` values (which is what happened when the slice was
    /// wrapped in an extra `Box::new(...)` before `Box::into_raw`).
    #[test]
    fn read_back_boxed_slice_u32_matches_original_data() {
        let original: Vec<u32> = vec![0xDEAD_BEEF, 0x1234_5678, 0, u32::MAX, 42];
        let len = original.len();

        // Mirror the (fixed) allocation pattern used in mboot_read_memory/mboot_get_property:
        // Box<[u32]> -> Box::into_raw -> cast to thin pointer for the C ABI.
        let boxed: Box<[u32]> = original.clone().into_boxed_slice();
        let ptr = Box::into_raw(boxed).cast::<u32>();

        // Simulate the C caller reading `response_words`/`response_words_len`.
        let read_back = unsafe { slice::from_raw_parts(ptr, len) };
        assert_eq!(
            read_back,
            original.as_slice(),
            "data read back through the raw pointer must match the original words, not fat-pointer metadata"
        );

        // Clean up using the length-aware free function (bug 2 fix).
        unsafe { free_boxed_slice(ptr, len) };
    }

    /// Same as above but for `Box<[u8]>`, mirroring the `bytes` field in
    /// `mboot_read_memory`'s response.
    #[test]
    fn read_back_boxed_slice_u8_matches_original_data() {
        let original: Vec<u8> = (0..=255u8).collect();
        let len = original.len();

        let boxed: Box<[u8]> = original.clone().into_boxed_slice();
        let ptr = Box::into_raw(boxed).cast::<u8>();

        let read_back = unsafe { slice::from_raw_parts(ptr, len) };
        assert_eq!(read_back, original.as_slice());

        unsafe { free_boxed_slice(ptr, len) };
    }

    /// `free_boxed_slice` must correctly handle the single-element case too (e.g. the
    /// "empty read" fallback in `mboot_read_memory`/`mboot_get_property` that returns a
    /// single zero word/byte).
    #[test]
    fn free_boxed_slice_single_element() {
        let boxed: Box<[u32]> = Box::new([0u32]);
        let ptr = Box::into_raw(boxed).cast::<u32>();
        unsafe { free_boxed_slice(ptr, 1) };
    }

    /// `free_boxed_slice` must be a no-op on a null pointer (used when e.g.
    /// `mboot_flash_read_once` returns zero bytes and stores a null `bytes` pointer).
    #[test]
    fn free_boxed_slice_null_is_noop() {
        unsafe { free_boxed_slice::<u32>(std::ptr::null_mut(), 0) };
    }

    /// `free_box_data` is used for single, non-slice values (e.g. `mboot_destroy` freeing
    /// the boxed `McuBoot<ProtocolImpl>>`); verify it round-trips a plain value correctly
    /// and is a no-op on null.
    #[test]
    fn free_box_data_roundtrip_and_null_is_noop() {
        let boxed: Box<u64> = Box::new(0x1122_3344_5566_7788);
        let ptr = Box::into_raw(boxed);
        unsafe { free_box_data(ptr) };

        unsafe { free_box_data::<u64>(std::ptr::null_mut()) };
    }
}
