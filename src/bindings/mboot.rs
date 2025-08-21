// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#![allow(
    clippy::doc_markdown,
    reason = "Some comments do not include any meaningful identifiers that would need to be enclosed in backticks."
)]

use core::panic;
use std::sync::Mutex;

use pyo3::{Py, prelude::*, types::PyType};

use crate::{
    CommunicationError, KeyProvisioningResponse, McuBoot,
    bindings::NOT_OPENED_ERROR,
    mboot::{ResultComm, ResultStatus},
    protocols::{ProtocolOpen, protocol_impl::ProtocolImpl, uart::UARTProtocol},
    tags::{
        command::{KeyProvOperation, KeyProvUserKeyType, TrustProvOperation},
        property::PropertyTagDiscriminants,
        status::StatusCode,
    },
};

use pyo3_stub_gen::derive::*;

#[gen_stub_pyclass]
#[pyclass(name = "McuBoot")]
struct McuBootPython {
    identifier: String,
    // Python can (and frequently) does pass class between threads, therefore each class needs to
    // implement Sync; on serialport, that can only be achieved with a mutex
    // you could also do it by making it linux only, TTYPort does implement sync unlike COMPort
    interface: Option<Mutex<McuBoot<ProtocolImpl>>>,
    #[pyo3(get)]
    status_code: StatusCode,
}

// TODO implement python exceptions for error
#[gen_stub_pymethods]
#[pymethods]
impl McuBootPython {
    #[new]
    fn py_new(identifier: String) -> Self {
        McuBootPython {
            identifier,
            interface: None,
            status_code: StatusCode::Success,
        }
    }

    #[getter]
    fn get_status_code_int(&self) -> usize {
        self.status_code as usize
    }

    #[getter]
    fn get_status_code_str(&self) -> String {
        self.status_code.to_string()
    }

    /// Connect to the device.
    fn open(&mut self) {
        let device = UARTProtocol::open(&self.identifier)
            .expect("device could not be opened")
            .into();
        let boot = McuBoot::new(device);
        self.interface = Some(Mutex::new(boot));
    }

    /// Disconnect from the device.
    fn close(&mut self) {
        self.interface = None;
    }

    #[pyo3(name = "__exit__", signature = (_exception_type = None, _exception_value = None, _traceback = None))]
    fn exit(
        &mut self,
        _exception_type: Option<Py<PyType>>,
        _exception_value: Option<Py<PyAny>>,
        _traceback: Option<Py<PyAny>>,
    ) {
        self.close();
    }

    #[pyo3(name = "__enter__")]
    fn enter(mut slf: PyRefMut<Self>) -> PyRefMut<Self> {
        slf.open();
        slf
    }

    /// Get specified property value.
    ///
    /// :param property: Property TAG (see `PropertyTag` Enum)
    /// :param index: External memory ID or internal memory region index (depends on property type), defaults to 0
    /// :return: list integers representing the property; None in case no response from device
    #[pyo3(signature = (property, index = None))]
    fn get_property(&mut self, property: PropertyTagDiscriminants, index: Option<u32>) -> Option<Vec<u32>> {
        let index = index.unwrap_or(0);
        let res = self.get_mut_interface().get_property(property, index);
        let res = self.process_result(res)?;
        self.status_code = res.status;
        Some(res.response_words.to_vec())
    }

    /// Set value of specified property.
    ///
    /// :param property: Property TAG (see `PropertyTag` enum)
    /// :param value: The value of selected property
    /// :return: False in case of any problem; True otherwise
    fn set_property(&mut self, property: PropertyTagDiscriminants, value: u32) {
        let res = self.get_mut_interface().set_property(property, value);
        if let Some(status) = self.process_result(res) {
            self.status_code = status;
        }
    }

    /// Erase complete flash memory without recovering flash security section.
    ///
    /// :param mem_id: Memory ID, defaults to 0
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (mem_id = None))]
    fn flash_erase_all(&mut self, mem_id: Option<u32>) -> bool {
        let mem_id = mem_id.unwrap_or(0);
        let res = self.get_mut_interface().flash_erase_all(mem_id);
        self.process_status_res(res)
    }

    /// Erase specified range of flash.
    ///
    /// :param address: Start address
    /// :param length: Count of bytes
    /// :param mem_id: Memory ID, defaults to 0
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (address, length, mem_id = None))]
    fn flash_erase_region(&mut self, address: u32, length: u32, mem_id: Option<u32>) -> bool {
        let mem_id = mem_id.unwrap_or(0);
        let res = self.get_mut_interface().flash_erase_region(address, length, mem_id);
        self.process_status_res(res)
    }

    /// Write data into MCU memory.
    ///
    /// :param address: Start address
    /// :param data: List of bytes
    /// :param `mem_id`: Memory ID, use `0` for internal memory, defaults to 0
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (address, data, mem_id = None))]
    fn write_memory(&mut self, address: u32, data: Vec<u8>, mem_id: Option<u32>) -> bool {
        let mem_id = mem_id.unwrap_or(0);
        let res = self.get_mut_interface().write_memory(address, mem_id, &data);
        self.process_status_res(res)
    }

    /// Read data from MCU memory.
    ///
    /// :param address: Start address
    /// :param length: Count of bytes
    /// :param `mem_id`: Memory ID, defaults to 0
    /// :return: Data read from the memory; None in case of a failure
    #[pyo3(signature = (address, length, mem_id = None))]
    fn read_memory(&mut self, address: u32, length: u32, mem_id: Option<u32>) -> Option<Vec<u8>> {
        let mem_id = mem_id.unwrap_or(0);
        let res = self.get_mut_interface().read_memory(address, length, mem_id);
        match self.process_result(res) {
            Some(data) => Some(data.bytes.to_vec()),
            None => None,
        }
    }

    /// Program fuse.
    ///
    /// :param address: Start address
    /// :param data: List of bytes
    /// :param mem_id: Memory ID, defaults to 0
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (address, data, mem_id = None))]
    fn fuse_program(&mut self, address: u32, data: Vec<u8>, mem_id: Option<u32>) -> bool {
        let mem_id = mem_id.unwrap_or(0);
        let res = self.get_mut_interface().fuse_program(address, mem_id, &data);
        self.process_status_res(res)
    }

    /// Read fuse.
    ///
    /// :param address: Start address
    /// :param length: Count of bytes
    /// :param mem_id: Memory ID, defaults to 0
    /// :return: Data read from the fuse; None in case of a failure
    #[pyo3(signature = (address, length, mem_id = None))]
    fn fuse_read(&mut self, address: u32, length: u32, mem_id: Option<u32>) -> Option<Vec<u8>> {
        let mem_id = mem_id.unwrap_or(0);
        let res = self.get_mut_interface().fuse_read(address, length, mem_id);
        match self.process_result(res) {
            Some(data) => Some(data.bytes.to_vec()),
            None => None,
        }
    }

    /// Execute program on a given address using the stack pointer.
    ///
    /// :param address: Jump address (must be word aligned)
    /// :param argument: Function arguments address
    /// :param sp: Stack pointer address
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (address, argument, sp))]
    fn execute(&mut self, address: u32, argument: u32, sp: u32) -> bool {
        let res = self.get_mut_interface().execute(address, argument, sp);
        self.process_status_res(res)
    }

    /// Call function on a given address.
    ///
    /// :param address: Call address (must be word aligned)
    /// :param argument: Function arguments address
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (address, argument))]
    fn call(&mut self, address: u32, argument: u32) -> bool {
        let res = self.get_mut_interface().call(address, argument);
        self.process_status_res(res)
    }

    /// Reset the MCU.
    ///
    /// :return: False in case of any problem; True otherwise
    fn reset(&mut self) -> bool {
        let res = self.get_mut_interface().reset();
        self.process_status_res(res)
    }

    /// Fill memory region with a pattern.
    ///
    /// :param start_address: Start address (must be word aligned)
    /// :param byte_count: Number of bytes to fill (must be word aligned)  
    /// :param pattern: 32-bit pattern to fill with
    /// :return: False in case of any problem; True otherwise
    fn fill_memory(&mut self, start_address: u32, byte_count: u32, pattern: u32) -> bool {
        let res = self.get_mut_interface().fill_memory(start_address, byte_count, pattern);
        self.process_status_res(res)
    }

    /// Erase all flash and recover security section.
    ///
    /// :return: False in case of any problem; True otherwise
    fn flash_erase_all_unsecure(&mut self) -> bool {
        let res = self.get_mut_interface().flash_erase_all_unsecure();
        self.process_status_res(res)
    }

    /// Configure external memory.
    ///
    /// :param memory_id: Memory ID to configure
    /// :param address: Address containing configuration data
    /// :return: False in case of any problem; True otherwise
    fn configure_memory(&mut self, memory_id: u32, address: u32) -> bool {
        let res = self.get_mut_interface().configure_memory(memory_id, address);
        self.process_status_res(res)
    }

    /// Receive and process a Secure Binary (SB) file.
    ///
    /// :param data: SB file data as list of bytes
    /// :return: False in case of any problem; True otherwise
    fn receive_sb_file(&mut self, data: Vec<u8>) -> bool {
        let res = self.get_mut_interface().receive_sb_file(&data);
        self.process_status_res(res)
    }

    /// Execute trust provisioning operation.
    ///
    /// :param operation: The trust provisioning operation to execute
    /// :return: Tuple of (success: bool, response_data: list of integers); (False, []) in case of failure
    fn trust_provisioning(&mut self, operation: &TrustProvOperation) -> (bool, Vec<u32>) {
        let res = self.get_mut_interface().trust_provisioning(operation);
        match self.process_result(res) {
            Some((status, response_words)) => {
                self.status_code = status;
                (true, response_words.to_vec())
            }
            None => (false, Vec::new()),
        }
    }

    /// Load image data directly to the device.
    ///
    /// :param data: Raw image data to be loaded as list of bytes
    /// :return: False in case of any problem; True otherwise
    fn load_image(&mut self, data: Vec<u8>) -> bool {
        let res = self.get_mut_interface().load_image(&data);
        self.process_status_res(res)
    }

    /// Read from MCU flash program once region (eFuse/OTP).
    ///
    /// :param index: Start index of the eFuse/OTP region
    /// :param count: Number of bytes to read (must be 4)
    /// :return: The read value as 32-bit integer; None in case of failure
    fn flash_read_once(&mut self, index: u32, count: u32) -> Option<u32> {
        let res = self.get_mut_interface().flash_read_once(index, count);
        self.process_result(res)
    }

    /// Write into MCU once program region (eFuse/OTP).
    ///
    /// :param index: Start index of the eFuse/OTP region
    /// :param count: Number of bytes to write (must be 4)
    /// :param data: 32-bit value to write
    /// :param verify: If true, reads back and verifies the written value, defaults to False
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (index, count, data, verify = None))]
    fn flash_program_once(&mut self, index: u32, count: u32, data: u32, verify: Option<bool>) -> bool {
        let verify = verify.unwrap_or(false);
        let res = self.get_mut_interface().flash_program_once(index, count, data, verify);
        self.process_status_res(res)
    }
    /// Key provisioning: Enroll Command (start PUF).
    ///
    /// :return: False in case of any problem; True otherwise
    fn kp_enroll(&mut self) -> bool {
        let operation = KeyProvOperation::Enroll;
        let res = self.get_mut_interface().key_provisioning(&operation);
        self.process_keyprov_result(res).0
    }

    /// Key provisioning: Generate Intrinsic Key.
    ///
    /// :param key_type: Type of the key
    /// :param key_size: Size of the key
    /// :return: False in case of any problem; True otherwise
    fn kp_set_intrinsic_key(&mut self, key_type: KeyProvUserKeyType, key_size: u32) -> bool {
        let operation = KeyProvOperation::SetKey { key_type, key_size };
        let res = self.get_mut_interface().key_provisioning(&operation);
        self.process_keyprov_result(res).0
    }

    /// Key provisioning: Write the key to a nonvolatile memory.
    ///
    /// :param memory_id: The memory ID, defaults to 0
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (memory_id = None))]
    fn kp_write_nonvolatile(&mut self, memory_id: Option<u32>) -> bool {
        let operation = KeyProvOperation::WriteKeyNonvolatile {
            memory_id: memory_id.unwrap_or(0),
        };
        let res = self.get_mut_interface().key_provisioning(&operation);
        self.process_keyprov_result(res).0
    }

    /// Key provisioning: Load the key from a nonvolatile memory to bootloader.
    ///
    /// :param memory_id: The memory ID, defaults to 0
    /// :return: False in case of any problem; True otherwise
    #[pyo3(signature = (memory_id = None))]
    fn kp_read_nonvolatile(&mut self, memory_id: Option<u32>) -> bool {
        let operation = KeyProvOperation::ReadKeyNonvolatile {
            memory_id: memory_id.unwrap_or(0),
        };
        let res = self.get_mut_interface().key_provisioning(&operation);
        self.process_keyprov_result(res).0
    }

    /// Key provisioning: Send the user key specified by <key_type> to bootloader.
    ///
    /// :param key_type: type of the user key, see enumeration for details
    /// :param key_data: binary content of the user key
    /// :return: False in case of any problem; True otherwise
    fn kp_set_user_key(&mut self, key_type: KeyProvUserKeyType, key_data: Vec<u8>) -> bool {
        let operation = KeyProvOperation::SetUserKey {
            key_type,
            key_data: key_data.into(),
        };
        let res = self.get_mut_interface().key_provisioning(&operation);
        self.process_keyprov_result(res).0
    }

    /// Key provisioning: Write key data into key store area.
    ///
    /// :param keystore_data: key store binary content to be written to processor
    /// :return: result of the operation; True means success
    fn kp_write_key_store(&mut self, keystore_data: Vec<u8>) -> bool {
        let operation = KeyProvOperation::WriteKeyStore {
            keystore_data: keystore_data.into(),
        };
        let res = self.get_mut_interface().key_provisioning(&operation);
        self.process_keyprov_result(res).0
    }

    /// Key provisioning: Read key data from key store area.
    ///
    /// :return: Key store data as bytes; None in case of failure
    fn kp_read_key_store(&mut self) -> Option<Vec<u8>> {
        let operation = KeyProvOperation::ReadKeyStore {
            file: String::new(),
            use_hexdump: false,
        };
        let res = self.get_mut_interface().key_provisioning(&operation);
        let (_, res) = self.process_keyprov_result(res);
        match res {
            Some(KeyProvisioningResponse::KeyStore { bytes, .. }) => Some(bytes.to_vec()),
            _ => None,
        }
    }
}

impl McuBootPython {
    fn get_mut_interface(&mut self) -> &mut McuBoot<ProtocolImpl> {
        self.interface.as_mut().expect(NOT_OPENED_ERROR).get_mut().unwrap()
    }

    fn process_keyprov_result(
        &mut self,
        packet: ResultComm<KeyProvisioningResponse>,
    ) -> (bool, Option<KeyProvisioningResponse>) {
        match packet {
            Ok(res @ KeyProvisioningResponse::KeyStore { status, .. }) => {
                self.status_code = status;
                (true, Some(res))
            }
            Ok(KeyProvisioningResponse::Status(status)) => {
                self.status_code = status;
                (true, None)
            }
            Err(_) => (false, None),
        }
    }

    fn process_result<T>(&mut self, packet: ResultComm<T>) -> Option<T> {
        match packet {
            Ok(res) => Some(res),
            Err(CommunicationError::UnexpectedStatus(status, _)) => {
                self.status_code = status;
                None
            }
            Err(err) => panic!("{}", err),
        }
    }
    fn process_status_res(&mut self, packet: ResultStatus) -> bool {
        let res = self.process_result(packet);
        match res {
            Some(status) => {
                self.status_code = status;
                true
            }
            None => false,
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<McuBootPython>()?;
    Ok(())
}
