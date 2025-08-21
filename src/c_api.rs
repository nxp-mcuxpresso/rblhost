// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#![warn(missing_docs)]

use crate::mboot::{McuBoot, ResultStatus, protocols::ProtocolOpen, tags::property::PropertyTagDiscriminants};
use crate::{
    protocols::{i2c::I2CProtocol, protocol_impl::ProtocolImpl, uart::UARTProtocol},
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
/// When positive, contains 32bit unsigned integer with data. When negative, indicates an error.
type ErrorData = i64;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
/// Struct filled by [`mboot_get_property`], containing data about a property.
pub struct CGetPropertyResponse {
    /// Received status code
    pub status: CStatus,
    /// Received reponse words
    pub response_words: *mut u32,
    /// Length of `response_words` in bytes
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
    /// Length of `response_words` in bytes
    pub response_words_len: usize,
    /// Received memory bytes
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
}

/// One of the passed pointers in function arguments was NULL.
pub const ERROR_NULL_POINTER_ARG: CStatus = -1;
/// Invalid property tag passed.
pub const ERROR_INVALID_PROPERTY_TAG: CStatus = -2;
/// Error occured while communication with the device.
pub const ERROR_COMMUNICATION_ERROR: CStatus = -3;

/// Get a mutable reference to [`McuBoot`] from mutable raw pointer.
///
/// # Safety
/// `mboot` must be a valid non-freed pointer.
unsafe fn get_mboot<'a>(mboot: *mut CMcuBoot) -> &'a mut McuBoot<ProtocolImpl> {
    unsafe { &mut *mboot.cast::<McuBoot<ProtocolImpl>>() }
}

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
            _ => "unknown status code",
        })
        .unwrap(),
    };
    text.into_raw()
}

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

/// Free any array allocated with [`Box`].
///
/// # Safety
/// `value` must be a valid non-freed pointer.
unsafe fn free_box_data<T>(value: *mut T) {
    if !value.is_null() {
        drop(unsafe { Box::from_raw(value) });
    }
}

#[unsafe(no_mangle)]
/// Create a new [`CMcuBoot`] instance from a device path.
///
/// Returns either a valid [`CMcuBoot`] instance or a NULL pointer, if any errors occur.
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
/// [`mboot_free_response_words`] function to free it.
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
/// [`mboot_free_response_words`] on `response_words` field **and** [`mboot_free_bytes`] on `bytes`
/// field to free them in any order you need.
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
            // Create copies of the response data
            let words = Box::new(res.response_words);
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
/// Returns a positive integer with a status code on success or a negative integer on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_flash_program_once(
    mboot: *mut CMcuBoot,
    index: u32,
    count: u32,
    data: u32,
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
/// Returns a positive 32bit unsigned integer with specified region's content or a negative integer
/// on error.
///
/// # Safety
/// `mboot` should be non-null and must be a valid pointer.
pub unsafe extern "C" fn mboot_flash_read_once(mboot: *mut CMcuBoot, index: u32, count: u32) -> ErrorData {
    if mboot.is_null() {
        return ERROR_NULL_POINTER_ARG.into();
    }
    let mboot = unsafe { get_mboot(mboot) };
    match mboot.flash_read_once(index, count) {
        Ok(res) => res.into(),
        Err(_) => ERROR_COMMUNICATION_ERROR.into(),
    }
}

#[unsafe(no_mangle)]
/// Free memory allocated for response words returned by a previous call.
///
/// # Safety
///
/// `words` should be non-null and must be a valid pointer returned by this API.
/// Passing an invalid or already-freed pointer results in undefined behavior.
pub unsafe extern "C" fn mboot_free_response_words(words: *mut u32) {
    unsafe { free_box_data(words) }
}

#[unsafe(no_mangle)]
/// Free memory allocated for a byte buffer returned by a previous call.
///
/// # Safety
///
/// `bytes` should be non-null and must be a valid pointer returned by this API.
/// Passing an invalid or already-freed pointer results in undefined behavior.
pub unsafe extern "C" fn mboot_free_bytes(bytes: *mut u8) {
    unsafe { free_box_data(bytes) }
}

#[unsafe(no_mangle)]
/// Free `response_words` and `bytes` in `response`.
///
/// # Safety
/// UB occurs if any data in `response` have already been freed.
pub unsafe extern "C" fn mboot_free_read_memory_response(response: *mut CReadMemoryResponse) {
    let response = unsafe { *response };
    unsafe {
        free_box_data(response.response_words);
        free_box_data(response.bytes);
    }
}
