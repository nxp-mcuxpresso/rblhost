// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use std::{io, time::Duration};

use crate::{mboot::ResultComm, parsers};
use color_print::cstr;
use hidapi::{DeviceInfo, HidApi, HidDevice};
use log::{debug, info};
use std::fmt::Debug;

use super::{CommunicationError, Protocol, ProtocolOpen};

/// Report IDs for USB-HID protocol as per NXP documentation
mod report {
    /// Command packet from host to device
    pub const CMD_OUT: u8 = 0x01;
    /// Data packet from host to device
    pub const DATA_OUT: u8 = 0x02;
    /// Response packet from device to host
    pub const CMD_IN: u8 = 0x03;
    /// Data packet from device to host
    pub const DATA_IN: u8 = 0x04;
}

/// Maximum packet size for USB transfers
const MAX_PACKET_SIZE: usize = 1024;

/// Known NXP USB Vendor IDs.
///
/// Used by [`USBProtocol::list_devices`] to filter the raw OS HID device list
/// down to devices that are plausibly NXP MCU bootloaders, instead of showing
/// every HID device on the system (mice, keyboards, etc.). Kept in sync with
/// the equivalent `NXP_VIDS` list in SPSDK (`spsdk/utils/usbfilter.py`).
pub const NXP_USB_VIDS: [u16; 4] = [0x1FC9, 0x15A2, 0x0471, 0x0D28];

/// Returns `true` if `vid` is a known NXP USB vendor ID (see [`NXP_USB_VIDS`]).
fn is_nxp_vid(vid: u16) -> bool {
    NXP_USB_VIDS.contains(&vid)
}

#[derive(Debug)]
pub struct USBProtocol {
    interface: String,
    device: HidDevice,
    timeout_ms: i32,
    polling_interval: Duration,
}

/// Information about a connected USB-HID device.
///
/// Used to disambiguate between several devices sharing the same VID/PID
/// (e.g. identical boards connected for parallel production programming).
/// Returned by [`USBProtocol::list_devices`] and primarily meant for the
/// `list-usb` CLI command and for building a `-u/--usb` selector.
#[derive(Clone, Debug)]
pub struct UsbDeviceInfo {
    /// USB Vendor ID
    pub vid: u16,
    /// USB Product ID
    pub pid: u16,
    /// OS-specific device path.
    ///
    /// On Windows this is a stable device instance path, unique per physical
    /// connection even across reconnects. **On Linux this is `/dev/hidrawN`,
    /// where `N` is assigned by the kernel at enumeration time and is NOT
    /// guaranteed stable across a device disconnect/reconnect or reset**
    /// (e.g. during provisioning). Prefer [`UsbDeviceInfo::topology_path`]
    /// on Linux when a stable identifier is required. On macOS,
    /// the path looks like `"DevSrvsID:4294969627"`, an `IORegistry` entry ID
    /// that is likewise reassigned on every reconnect or reset, so prefer
    /// [`UsbDeviceInfo::topology_path`] there too. Several HID *interfaces*
    /// exposed by the same physical device can also report the same macOS
    /// path, so it may not be unique to a single `hidapi` device entry.
    pub path: String,
    /// Physical USB topology (port-chain) path, e.g. `"1-1.4.3"`.
    ///
    /// Identifies the device by its physical location in the USB hub
    /// hierarchy rather than by enumeration order. Unlike [`path`](Self::path)
    /// on Linux, this remains stable across disconnect/reconnect and device
    /// reset events, as long as the physical cabling/hub topology doesn't
    /// change. Available on Linux (from sysfs) and macOS (decoded from the
    /// USB `locationID`, e.g. `0x14320000` -> `"20-3.2"`); `None` on Windows,
    /// where [`path`](Self::path) is already stable and should be used instead.
    pub topology_path: Option<String>,
    /// Serial number string reported by the device, if any
    pub serial_number: Option<String>,
    /// Product string reported by the device, if any
    pub product_string: Option<String>,
    /// Manufacturer string reported by the device, if any
    pub manufacturer_string: Option<String>,
}

impl std::fmt::Display for UsbDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "0x{:04X}:0x{:04X}  path={}  topology={}  serial={}  product={}",
            self.vid,
            self.pid,
            self.path,
            self.topology_path.as_deref().unwrap_or("N/A"),
            self.serial_number.as_deref().unwrap_or("N/A"),
            self.product_string.as_deref().unwrap_or("N/A"),
        )
    }
}

impl From<&DeviceInfo> for UsbDeviceInfo {
    fn from(info: &DeviceInfo) -> Self {
        let path = info.path().to_string_lossy().into_owned();
        UsbDeviceInfo {
            vid: info.vendor_id(),
            pid: info.product_id(),
            topology_path: usb_topology_path_from_hid_path(&path),
            path,
            serial_number: info.serial_number().map(str::to_owned),
            product_string: info.product_string().map(str::to_owned),
            manufacturer_string: info.manufacturer_string().map(str::to_owned),
        }
    }
}

impl UsbDeviceInfo {
    /// Build a `-u/--usb` argument (`<vid>,<pid>,<path>`) that identifies this device, ready to
    /// copy-paste directly, e.g. in `rblhost -u "<value>" get-property 1`.
    ///
    /// Always includes the full OS device path as the selector (rather than trying to find
    /// the shortest unambiguous form), to keep the output predictable regardless of how many
    /// other devices are connected.
    ///
    /// Caveats on the OS path's uniqueness (none of which are specific to this method, they
    /// apply to [`path`](Self::path) itself and thus to any selector built from it):
    /// - On Linux, the path (`/dev/hidrawN`) is only unique for as long as the device stays
    ///   connected without being re-enumerated; see [`topology_path`](Self::topology_path) and
    ///   [`usb_topology_arg`](Self::usb_topology_arg) for a selector that survives
    ///   disconnect/reconnect/reset.
    /// - On macOS, the path (`DevSrvsID:<id>`) changes on every reconnect/reset as well; use
    ///   [`usb_topology_arg`](Self::usb_topology_arg) instead. Several HID interfaces exposed
    ///   by the *same physical device* can also share the
    ///   same path (e.g. a composite device also exposing a non-bootloader interface), so the
    ///   resulting argument may not be unique to a single `hidapi` device entry even though it
    ///   does identify a single physical device.
    #[must_use]
    pub fn usb_arg(&self) -> String {
        format!("0x{:04X},0x{:04X},{}", self.vid, self.pid, self.path)
    }

    /// Build a `-u/--usb` argument using the stable USB topology (port-chain) path instead of
    /// the OS device path, e.g. `"0x1FC9,0x0135,1-1.4.3"`.
    ///
    /// Returns `None` if [`topology_path`](Self::topology_path) couldn't be determined (always
    /// the case on Windows, or if the sysfs/IOKit topology lookup failed on Linux/macOS).
    /// Unlike [`usb_arg`](Self::usb_arg), the value returned here remains valid across
    /// disconnect/reconnect and device reset events, as long as the physical USB
    /// cabling/hub topology doesn't change.
    #[must_use]
    pub fn usb_topology_arg(&self) -> Option<String> {
        self.topology_path
            .as_deref()
            .map(|topology| format!("0x{:04X},0x{:04X},{}", self.vid, self.pid, topology))
    }
}

/// Resolves the stable USB topology (port-chain) path, e.g. `"1-1.4.3"`, for the HID device at
/// `hid_path` (a hidapi OS device path, `/dev/hidrawN` on Linux).
///
/// This identifies a device by its physical location in the USB hub hierarchy, which (unlike
/// `/dev/hidrawN`, whose `N` is assigned by the kernel at enumeration time) stays stable across
/// disconnect/reconnect and device reset events as long as the physical cabling/hub topology
/// doesn't change. This allows deterministically targeting one of several identical boards
/// attached through a USB hub tree, e.g. in manufacturing/provisioning setups.
///
/// Implemented by walking `/sys/class/hidraw/<N>/device` up through its ancestor sysfs
/// directories until one is found whose name matches the kernel's USB device naming scheme
/// (`<bus>-<port>[.<port>]*`, e.g. `1-1.4.3`); that name *is* the topology path. Returns `None`
/// on non-Linux platforms, or if `hid_path` isn't a `/dev/hidrawN` path, or if the sysfs lookup
/// fails for any reason (e.g. the device was unplugged between listing and this call, or sysfs
/// isn't mounted) -- the topology path is a best-effort convenience, callers must be prepared
/// to fall back to [`UsbDeviceInfo::path`] or VID/PID+serial selection.
#[cfg(target_os = "linux")]
fn usb_topology_path_from_hid_path(hid_path: &str) -> Option<String> {
    let hidraw_name = hid_path.strip_prefix("/dev/")?;
    if !hidraw_name.starts_with("hidraw") {
        return None;
    }

    let device_link = std::path::Path::new("/sys/class/hidraw")
        .join(hidraw_name)
        .join("device");
    let resolved = std::fs::canonicalize(device_link).ok()?;

    // Walk up the resolved sysfs path (.../usb1/1-1/1-1.4/1-1.4.3/1-1.4.3:1.0/<hid>/hidraw/hidrawN)
    // looking for an ancestor directory name that matches the USB device naming scheme used by
    // the kernel for topology: "<root hub number>-<port>[.<port>]*", e.g. "1-1.4.3". USB
    // interface directories (e.g. "1-1.4.3:1.0") and the root hub itself (e.g. "usb1") don't
    // match this pattern and are skipped.
    resolved
        .ancestors()
        .filter_map(|p| p.file_name())
        .filter_map(|name| name.to_str())
        .find(|name| is_usb_topology_name(name))
        .map(str::to_owned)
}

/// Resolves the stable USB topology (port-chain) path for the HID device at `hid_path` (a hidapi
/// macOS device path, `"DevSrvsID:<registry entry ID>"`).
///
/// The entry ID in the path is assigned anew every time the device enumerates, so it changes on
/// reset/reconnect. The USB `locationID` of the device, found by walking up the `IOService` plane
/// from the HID entry, encodes its physical bus/port chain instead; it is converted to the same
/// `<bus>-<port>[.<port>]*` form as on Linux by [`topology_path_from_location_id`]. Returns `None`
/// if `hid_path` isn't a `DevSrvsID:` path or the `IOKit` lookup fails.
#[cfg(target_os = "macos")]
fn usb_topology_path_from_hid_path(hid_path: &str) -> Option<String> {
    let entry_id = hid_path.strip_prefix("DevSrvsID:")?.parse::<u64>().ok()?;
    topology_path_from_location_id(macos::usb_location_id(entry_id)?)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn usb_topology_path_from_hid_path(_hid_path: &str) -> Option<String> {
    None
}

/// Converts a macOS USB `locationID` to a `<bus>-<port>[.<port>]*` topology path, e.g.
/// `0x14320000` -> `"20-3.2"`.
///
/// The top byte of a `locationID` is the bus number, and each following hex digit is the port
/// number at the next hub level, terminated by the first zero digit. Returns `None` for IDs
/// without any port (e.g. `0`, reported by built-in devices that aren't on a USB bus).
#[cfg(any(target_os = "macos", test))]
fn topology_path_from_location_id(location_id: u32) -> Option<String> {
    let ports: Vec<String> = (0..6)
        .map(|level| (location_id >> (20 - 4 * level)) & 0xF)
        .take_while(|&port| port != 0)
        .map(|port| port.to_string())
        .collect();
    if ports.is_empty() {
        return None;
    }
    Some(format!("{}-{}", location_id >> 24, ports.join(".")))
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void};

    type CFTypeRef = *const c_void;
    type IoObject = u32;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_SINT64_TYPE: isize = 4;
    const K_IO_REGISTRY_ITERATE_RECURSIVELY: u32 = 1;
    const K_IO_REGISTRY_ITERATE_PARENTS: u32 = 2;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IORegistryEntryIDMatching(entry_id: u64) -> *mut c_void;
        fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> IoObject;
        fn IORegistryEntrySearchCFProperty(
            entry: IoObject,
            plane: *const c_char,
            key: CFTypeRef,
            allocator: CFTypeRef,
            options: u32,
        ) -> CFTypeRef;
        fn IOObjectRelease(object: IoObject) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(allocator: CFTypeRef, c_str: *const c_char, encoding: u32) -> CFTypeRef;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFNumberGetValue(number: CFTypeRef, number_type: isize, value: *mut c_void) -> u8;
        fn CFRelease(cf: CFTypeRef);
    }

    /// Looks up the USB `locationID` of the nearest USB ancestor of the `IORegistry` entry with
    /// the given ID, without opening the device.
    pub(super) fn usb_location_id(entry_id: u64) -> Option<u32> {
        // SAFETY: plain IOKit/CoreFoundation calls; every object obtained here is checked for
        // null and released exactly once. IOServiceGetMatchingService consumes `matching`.
        unsafe {
            let matching = IORegistryEntryIDMatching(entry_id);
            if matching.is_null() {
                return None;
            }
            let service = IOServiceGetMatchingService(0, matching);
            if service == 0 {
                return None;
            }
            let key = CFStringCreateWithCString(std::ptr::null(), c"locationID".as_ptr(), K_CF_STRING_ENCODING_UTF8);
            let value = if key.is_null() {
                std::ptr::null()
            } else {
                let value = IORegistryEntrySearchCFProperty(
                    service,
                    c"IOService".as_ptr(),
                    key,
                    std::ptr::null(),
                    K_IO_REGISTRY_ITERATE_RECURSIVELY | K_IO_REGISTRY_ITERATE_PARENTS,
                );
                CFRelease(key);
                value
            };
            IOObjectRelease(service);
            if value.is_null() {
                return None;
            }
            let mut location_id: i64 = 0;
            let ok = CFGetTypeID(value) == CFNumberGetTypeID()
                && CFNumberGetValue(value, K_CF_NUMBER_SINT64_TYPE, (&raw mut location_id).cast()) != 0;
            CFRelease(value);
            // Apple Silicon reports some locationIDs sign-extended to 64 bits (e.g.
            // 0xFFFFFFFF80000000); only the low 32 bits are meaningful.
            if ok {
                u32::try_from(location_id & 0xFFFF_FFFF).ok()
            } else {
                None
            }
        }
    }
}

/// Returns `true` if `name` matches the kernel's USB device topology naming scheme:
/// `<bus>-<port>[.<port>]*`, e.g. `"1-1"`, `"1-1.4"`, `"1-1.4.3"`. Rejects USB interface
/// directory names such as `"1-1.4.3:1.0"` (which additionally contain a `':'`) and anything
/// else that doesn't fit the pattern (e.g. `"usb1"`, `"hidraw0"`).
#[cfg(target_os = "linux")]
fn is_usb_topology_name(name: &str) -> bool {
    let Some((bus, ports)) = name.split_once('-') else {
        return false;
    };
    if bus.is_empty() || !bus.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    !ports.is_empty()
        && ports
            .split('.')
            .all(|port| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
}

/// Parsed form of a `-u/--usb` device identifier.
///
/// Supports the original `<vid>` / `<vid>,<pid>` forms (opens the first
/// matching device, as before, with no guarantee which one that is if several
/// identical devices are connected), an additional `<vid>,<pid>,<selector>`
/// form used to target one specific device when several identical boards
/// (same VID/PID) are connected at once, and a bare `<selector>` form (no
/// VID/PID at all) for when the device's OS path, serial number, or USB
/// topology path alone is enough to uniquely identify it. In all cases,
/// `<selector>` matches case-insensitively against either a substring of the
/// device's OS path, its exact serial number, or its exact USB topology path.
///
/// Caveat: a bare `<selector>` that happens to look like a plain number
/// (decimal, or hex without a `0x` prefix containing only `0`-`9`/`a`-`f`) is
/// always parsed as `VidPid` with that number as the VID, never as
/// `PathOrSerial` -- e.g. a numeric serial number `"1234"` is interpreted as
/// `vid=0x04D2`, and a hex-looking serial `"ABCD"` as `vid=0xABCD`. Numeric or
/// short hex-like serial numbers must therefore always be given with an
/// explicit (even if wildcard, `pid=0`) VID/PID prefix, e.g.
/// `"0x1FC9,0,1234"`, to be unambiguously treated as a selector rather than a
/// VID.
#[derive(Debug, PartialEq, Eq)]
enum UsbSelector {
    /// Open the first device matching VID/PID (PID `0` matches any product)
    VidPid { vid: u16, pid: u16 },
    /// Open the device matching VID/PID (PID `0` matches any product) whose
    /// path (substring), serial number (exact), or USB topology path (exact)
    /// matches `selector`, case-insensitively
    VidPidWithSelector { vid: u16, pid: u16, selector: String },
    /// Open the device (any VID/PID) whose path (substring), serial number
    /// (exact), or USB topology path (exact) matches `selector`,
    /// case-insensitively. Used when the input can't be parsed as a VID
    /// number, e.g. a raw OS device path copied from `list-usb` or Device
    /// Manager.
    PathOrSerial { selector: String },
}

impl ProtocolOpen for USBProtocol {
    fn open(identifier: &str) -> ResultComm<Self> {
        Self::open_with_options(identifier, 0, Duration::from_secs(5), Duration::from_millis(1))
    }

    fn open_with_options(
        identifier: &str,
        _baudrate: u32, // Not used for USB
        timeout: Duration,
        polling_interval: Duration,
    ) -> ResultComm<Self> {
        let selector = parse_usb_identifier(identifier)?;

        // Initialize HidApi
        let api =
            HidApi::new().map_err(|e| CommunicationError::ParseError(format!("Failed to initialize HID API: {e}")))?;

        let device = open_selected_device(&api, &selector)?;

        // Convert timeout to i32, clamping if necessary
        let timeout_ms = timeout.as_millis().try_into().unwrap_or(i32::MAX);

        let usb_protocol = USBProtocol {
            interface: identifier.to_owned(),
            device,
            timeout_ms,
            polling_interval,
        };

        info!(
            "Opened USB-HID device {} with {}ms timeout",
            usb_protocol.interface,
            timeout.as_millis()
        );

        Ok(usb_protocol)
    }
}

impl Protocol for USBProtocol {
    fn get_polling_interval(&self) -> Duration {
        self.polling_interval
    }

    fn get_timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.try_into().expect("negative timeout in USB"))
    }

    fn get_identifier(&self) -> &str {
        &self.interface
    }

    fn read(&mut self, bytes: usize) -> ResultComm<Vec<u8>> {
        let mut buf = vec![0u8; bytes];
        self.read_usb(&mut buf)?;
        Ok(buf)
    }
    fn write_packet_raw(&mut self, data: &[u8]) -> ResultComm<()> {
        // For USB-HID, we need to extract the command data from the UART framing
        // UART frame format: [5A, cmd_type, len_lsb, len_msb, crc_lsb, crc_msb, ...data...]
        if data.len() < 6 || data[0] != 0x5A {
            return Err(CommunicationError::InvalidHeader);
        }

        let cmd_type = data[1];
        let data_len = u16::from_le_bytes([data[2], data[3]]) as usize;

        if data.len() < 6 + data_len {
            return Err(CommunicationError::InvalidData);
        }

        // Extract the command data (without UART framing and CRC)
        // Skip the UART header (4 bytes) and CRC (2 bytes)
        let cmd_data = &data[6..6 + data_len];

        // Determine report ID based on packet type
        let report_id = match cmd_type {
            0xA4 => report::CMD_OUT,  // Command packet
            0xA5 => report::DATA_OUT, // Data packet
            _ => return Err(CommunicationError::InvalidHeader),
        };

        // Create a generic HID report
        let mut report = vec![0u8; 4 + cmd_data.len()]; // 4 bytes for header + data

        // Set report header
        report[0] = report_id;
        report[1] = 0x00; // Padding (should be 0)
        report[2] = (cmd_data.len() & 0xFF) as u8;
        report[3] = ((cmd_data.len() >> 8) & 0xFF) as u8;

        // Copy command data
        report[4..4 + cmd_data.len()].copy_from_slice(cmd_data);

        // Write the report
        self.write_usb(&report)?;

        Ok(())
    }
    //

    fn read_packet_raw(&mut self, _: u8) -> ResultComm<Vec<u8>> {
        // Read the initial response
        let mut report = vec![0u8; MAX_PACKET_SIZE];
        let size = self
            .device
            .read_timeout(&mut report, self.timeout_ms)
            .map_err(|e| CommunicationError::IOError(io::Error::other(e.to_string())))?;

        debug!("{}: Read {} bytes: {:02X?}", cstr!("<r!>RX"), size, &report[..size]);

        if size < 4 {
            return Err(CommunicationError::InvalidHeader);
        }

        // Extract report ID and packet length
        let report_id = report[0];
        let packet_length = u16::from_le_bytes([report[2], report[3]]) as usize;

        if packet_length == 0 {
            // error!(cstr!("<r!>RX</>: Data aborted by sender!"));
            return Err(CommunicationError::Aborted);
        }

        // Check if this is a command response (report ID 0x03)
        if report_id == report::CMD_IN {
            // For other command responses, extract the payload
            let mut response = Vec::new();

            // Extract the command tag and other fields
            response.extend_from_slice(&report[4..4 + packet_length]);

            debug!("Constructed response: {response:02X?}");

            return Ok(response);
        } else if report_id == report::DATA_IN {
            // Data packet - extract the data portion
            if size >= 4 + packet_length {
                return Ok(report[4..4 + packet_length].to_vec());
            }
        }

        // For other packet types, just return the data portion
        if size > 4 {
            Ok(report[4..size].to_vec())
        } else {
            Ok(Vec::new())
        }
    }
}

impl USBProtocol {
    /// List currently connected USB-HID devices.
    ///
    /// By default, only devices whose VID matches a known NXP vendor ID
    /// (see [`NXP_USB_VIDS`]) are returned, to avoid cluttering the output
    /// with unrelated HID devices (mice, keyboards, etc.) present on the
    /// system. Pass `all = true` to return every HID device the OS reports,
    /// unfiltered.
    ///
    /// This is primarily intended to help a user find the `path` or `serial`
    /// of a specific board when several identical devices (same VID/PID) are
    /// connected, for use as the `<selector>` part of a
    /// `-u/--usb <vid>,<pid>,<selector>` identifier.
    ///
    /// # Errors
    /// Returns an error if the underlying HID API fails to initialize.
    pub fn list_devices(all: bool) -> ResultComm<Vec<UsbDeviceInfo>> {
        let api =
            HidApi::new().map_err(|e| CommunicationError::ParseError(format!("Failed to initialize HID API: {e}")))?;
        Ok(api
            .device_list()
            .filter(|info| all || is_nxp_vid(info.vendor_id()))
            .map(UsbDeviceInfo::from)
            .collect())
    }

    fn read_usb(&mut self, buf: &mut [u8]) -> Result<(), io::Error> {
        match self.device.read(buf) {
            Ok(size) => {
                debug!("{}: Read {} bytes: {:02X?}", cstr!("<r!>RX"), size, &buf[..size]);
                Ok(())
            }
            Err(e) => Err(io::Error::other(e.to_string())),
        }
    }
    fn write_usb(&self, buf: &[u8]) -> Result<(), io::Error> {
        debug!("{}: {:02X?}", cstr!("<g!>TX"), buf);

        match self.device.write(buf) {
            Ok(written) => {
                // Platform-specific validation
                #[cfg(target_os = "windows")]
                {
                    // Windows HID might report different sizes due to report descriptors
                    // As long as write succeeded (written > 0), consider it successful
                    if written > 0 {
                        Ok(())
                    } else {
                        Err(io::Error::other("Failed to write to USB device"))
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // On other platforms, we expect the exact byte count
                    if written == buf.len() {
                        Ok(())
                    } else {
                        Err(io::Error::other(format!(
                            "Failed to write all bytes: wrote {} of {}",
                            written,
                            buf.len()
                        )))
                    }
                }
            }
            Err(e) => Err(io::Error::other(e.to_string())),
        }
    }
}

// Helper functions

/// Returns `true` if `info`'s OS path matches `selector` (exactly if `exact_path`, otherwise
/// as a substring of it), if its serial number matches `selector` exactly, or if its USB
/// topology path (see [`usb_topology_path_from_hid_path`]) matches `selector` exactly. All
/// comparisons are case-insensitive.
fn device_matches_selector(info: &DeviceInfo, selector: &str, selector_lower: &str, exact_path: bool) -> bool {
    let path = info.path().to_string_lossy();
    let path_matches = if exact_path {
        path.eq_ignore_ascii_case(selector)
    } else {
        path.to_lowercase().contains(selector_lower)
    };
    path_matches
        || info
            .serial_number()
            .is_some_and(|serial| serial.eq_ignore_ascii_case(selector))
        || usb_topology_path_from_hid_path(&path).is_some_and(|topology| topology.eq_ignore_ascii_case(selector))
}

/// Open the device described by `selector`.
///
/// For [`UsbSelector::VidPid`], behaves exactly as before: opens the first
/// device matching VID/PID as reported by the OS/hidapi (no guarantee which
/// one that is if several identical devices are connected). No ambiguity
/// check is done here on purpose, to keep this form backward-compatible with
/// its pre-existing "open first match" behavior.
///
/// For [`UsbSelector::VidPidWithSelector`] and [`UsbSelector::PathOrSerial`],
/// scans all connected devices (restricted to matching VID/PID for the
/// former; every HID device for the latter) and matches `selector` against
/// each one's OS path (substring), serial number (exact), or USB topology
/// (port-chain) path (exact, Linux/macOS), all case-insensitively. Unlike
/// [`UsbSelector::VidPid`], disambiguating by selector is the entire point of
/// these two forms, so **more than one match is treated as an error** (listing
/// every ambiguous candidate) rather than silently opening the first one --
/// a substring selector such as a single digit can otherwise match several
/// devices at once (e.g. Linux `/dev/hidraw1` also matches `/dev/hidraw1[0-9]`)
/// and parallel-programming callers must be able to trust that they always
/// open the one board they asked for. Returns an error listing the available
/// candidates if none match either.
fn open_selected_device(api: &HidApi, selector: &UsbSelector) -> ResultComm<HidDevice> {
    match *selector {
        // NOTE: `pid == 0` is documented as "match any product", but `HidApi::open`
        // performs an exact PID match, so `open(vid, 0)` would never find a real
        // device whose PID isn't literally 0. Filter the device list ourselves
        // instead, so `pid == 0` really means "any PID" (and an exact PID still
        // matches exactly).
        UsbSelector::VidPid { vid, pid } => {
            let matched = api
                .device_list()
                .find(|info| info.vendor_id() == vid && (pid == 0 || info.product_id() == pid));

            match matched {
                Some(info) => info
                    .open_device(api)
                    .map_err(|e| CommunicationError::ParseError(format!("Failed to open USB device: {e}"))),
                None => Err(CommunicationError::ParseError(format!(
                    "No USB-HID device matching vid=0x{vid:04X}{} found. Use the 'list-usb' command to see all \
                     connected devices.",
                    if pid == 0 {
                        String::new()
                    } else {
                        format!(" pid=0x{pid:04X}")
                    }
                ))),
            }
        }
        UsbSelector::VidPidWithSelector {
            vid,
            pid,
            selector: ref selector_str,
        } => {
            // Same `pid == 0` => "any PID" convention as `VidPid` above, so that e.g.
            // "0x1FC9,0,<selector>" behaves consistently with "0x1FC9,0" rather than
            // silently matching nothing (hidapi/real devices never report PID 0).
            let selector_lower = selector_str.to_lowercase();
            let candidates: Vec<&DeviceInfo> = api
                .device_list()
                .filter(|info| info.vendor_id() == vid && (pid == 0 || info.product_id() == pid))
                .collect();

            open_unique_match(api, &candidates, selector_str, &selector_lower, || {
                format!("vid=0x{vid:04X} pid=0x{pid:04X} and selector '{selector_str}'")
            })
        }
        UsbSelector::PathOrSerial {
            selector: ref selector_str,
        } => {
            let selector_lower = selector_str.to_lowercase();
            let candidates: Vec<&DeviceInfo> = api.device_list().collect();

            open_unique_match(api, &candidates, selector_str, &selector_lower, || {
                format!("path/serial/topology '{selector_str}'")
            })
        }
    }
}

/// Shared matching logic for [`UsbSelector::VidPidWithSelector`] and
/// [`UsbSelector::PathOrSerial`]: finds every device among `candidates` that
/// matches `selector`/`selector_lower` (see [`device_matches_selector`]), and:
/// - prefers devices whose OS path equals the selector exactly, so a full path copied from
///   `list-usb` is never rejected as ambiguous just because it is a substring of another
///   device's path (e.g. `/dev/hidraw1` vs. `/dev/hidraw10`); substring matching is only
///   used when no device matches exactly,
/// - opens it if exactly one matched,
/// - errors out listing all ambiguous candidates if more than one matched
///   (disambiguation is the whole point of these two selector forms, so
///   silently opening "the first one" would defeat it -- see
///   [`open_selected_device`]),
/// - errors out listing all same-VID/PID candidates (if any) if none matched.
///
/// `describe_target` lazily builds the "no device matching ..." message
/// prefix (only called when actually needed, i.e. on the error paths).
fn open_unique_match(
    api: &HidApi,
    candidates: &[&DeviceInfo],
    selector_str: &str,
    selector_lower: &str,
    describe_target: impl FnOnce() -> String,
) -> ResultComm<HidDevice> {
    let find_matches = |exact_path| -> Vec<&&DeviceInfo> {
        candidates
            .iter()
            .filter(|info| device_matches_selector(info, selector_str, selector_lower, exact_path))
            .collect()
    };
    let mut matches = find_matches(true);
    if matches.is_empty() {
        matches = find_matches(false);
    }

    match matches.as_slice() {
        [info] => info
            .open_device(api)
            .map_err(|e| CommunicationError::ParseError(format!("Failed to open USB device: {e}"))),
        [] => {
            let available: Vec<String> = candidates
                .iter()
                .map(|info| UsbDeviceInfo::from(*info).to_string())
                .collect();
            Err(CommunicationError::ParseError(format!(
                "No USB-HID device matching {}. Use the 'list-usb' command to see all connected devices.{}",
                describe_target(),
                if available.is_empty() {
                    String::new()
                } else {
                    format!(" Other connected devices: [{}]", available.join(", "))
                }
            )))
        }
        _ => {
            let ambiguous: Vec<String> = matches
                .iter()
                .map(|info| UsbDeviceInfo::from(**info).to_string())
                .collect();
            Err(CommunicationError::ParseError(format!(
                "Selector '{selector_str}' matches {} devices, refusing to guess which one to open: [{}]. Use a \
                 more specific selector (e.g. the full OS path, serial number, or USB topology path from \
                 'list-usb').",
                matches.len(),
                ambiguous.join(", ")
            )))
        }
    }
}

fn parse_usb_identifier(identifier: &str) -> ResultComm<UsbSelector> {
    // NOTE: both ':' and ',' are accepted as the vid/pid separator (and as the
    // pid/selector separator), for compatibility with historical "<vid>:<pid>"
    // usage. To avoid mis-splitting a bare path/serial selector that happens
    // to contain a literal ':' or ',' (e.g. a macOS HID path such as
    // "DevSrvsID:4294969627"), we only treat the identifier as containing a
    // vid/pid prefix if its first ':'/','-separated part actually parses as a
    // number; otherwise the *whole* identifier is used verbatim as a bare
    // path/serial/topology-path selector, with no splitting at all. Windows
    // (`\\?\HID#...`) and typical Linux paths/serials/topology paths don't
    // contain ':' or ',' either, so this only matters on macOS.
    let parts: Vec<&str> = identifier.splitn(3, [':', ',']).collect();

    let vid_str = parts[0];
    if vid_str.is_empty() {
        return Err(CommunicationError::ParseError(format!(
            "Invalid USB identifier: {identifier} (empty)"
        )));
    }
    let Ok(vid) = parse_number_string(vid_str) else {
        // The first part isn't a valid VID number: treat the *entire* identifier
        // (not just `vid_str`) as a raw OS device path (substring match), exact
        // serial number, or exact USB topology path, e.g. a value copied
        // directly from `list-usb` or the OS's device manager.
        return Ok(UsbSelector::PathOrSerial {
            selector: identifier.to_owned(),
        });
    };

    match parts.as_slice() {
        [_] => {
            // Use 0 as default PID, which will match any device with the specified VID
            Ok(UsbSelector::VidPid { vid, pid: 0 })
        }
        [_, pid_str] => {
            let pid = parse_number_string(pid_str)
                .map_err(|_| CommunicationError::ParseError(format!("Invalid PID: {pid_str}")))?;
            Ok(UsbSelector::VidPid { vid, pid })
        }
        [_, pid_str, selector] => {
            let pid = parse_number_string(pid_str)
                .map_err(|_| CommunicationError::ParseError(format!("Invalid PID: {pid_str}")))?;
            if selector.is_empty() {
                return Err(CommunicationError::ParseError(format!(
                    "Invalid USB identifier: {identifier} (selector part is empty)"
                )));
            }
            Ok(UsbSelector::VidPidWithSelector {
                vid,
                pid,
                selector: (*selector).to_owned(),
            })
        }
        _ => Err(CommunicationError::ParseError(format!(
            "Invalid USB identifier: {identifier}"
        ))),
    }
}

/// Parse a number string that can be either decimal or hexadecimal.
///
/// Delegates to [`parsers::parse_number`] (same `0x`-prefixed-hex-or-decimal rules used
/// everywhere else in the CLI; the trimming and uppercase-`0X`-prefix normalization is done
/// here at the call site, as `parse_number` itself only recognizes a lowercase `0x` prefix),
/// with a fallback to hex-without-prefix (e.g. `"ABCD"`) for backward compatibility with
/// pre-existing bare-hex VID/PID input.
fn parse_number_string(s: &str) -> Result<u16, String> {
    let trimmed = s.trim();
    let lowercased_prefix = trimmed.strip_prefix("0X").map(|rest| format!("0x{rest}"));
    let normalized = lowercased_prefix.as_deref().unwrap_or(trimmed);

    parsers::parse_number::<u16>(normalized).or_else(|_| {
        u16::from_str_radix(trimmed, 16).map_err(|_| format!("number '{trimmed}' is invalid or too large!"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vid_only() {
        let selector = parse_usb_identifier("0x1FC9").unwrap();
        assert_eq!(selector, UsbSelector::VidPid { vid: 0x1FC9, pid: 0 });
    }

    #[test]
    fn test_parse_vid_uppercase_prefix() {
        // `parse_number_string` must still accept an uppercase "0X" prefix, same as before
        // it was simplified to delegate to `parsers::parse_number` (which only recognizes a
        // lowercase "0x" prefix on its own).
        let selector = parse_usb_identifier("0X1FC9").unwrap();
        assert_eq!(selector, UsbSelector::VidPid { vid: 0x1FC9, pid: 0 });
    }

    #[test]
    fn test_parse_vid_hex_without_prefix() {
        // Bare hex-looking VID (no "0x"/"0X" prefix, but containing a-f so it can't be parsed
        // as decimal) must still be accepted, e.g. copy-pasted straight from Device
        // Manager/`lsusb` output.
        let selector = parse_usb_identifier("1FC9").unwrap();
        assert_eq!(selector, UsbSelector::VidPid { vid: 0x1FC9, pid: 0 });
    }

    #[test]
    fn test_parse_vid_pid_comma() {
        let selector = parse_usb_identifier("0x1FC9,0x0135").unwrap();
        assert_eq!(
            selector,
            UsbSelector::VidPid {
                vid: 0x1FC9,
                pid: 0x0135
            }
        );
    }

    #[test]
    fn test_parse_vid_pid_colon() {
        let selector = parse_usb_identifier("0x1FC9:0x0135").unwrap();
        assert_eq!(
            selector,
            UsbSelector::VidPid {
                vid: 0x1FC9,
                pid: 0x0135
            }
        );
    }

    #[test]
    fn test_parse_vid_pid_decimal() {
        let selector = parse_usb_identifier("8137,309").unwrap();
        assert_eq!(selector, UsbSelector::VidPid { vid: 8137, pid: 309 });
    }

    #[test]
    fn test_parse_vid_pid_with_path_selector() {
        let selector = parse_usb_identifier("0x1FC9,0x0135,HID\\VID_1FC9&PID_0135\\6&7909BF&0&0000").unwrap();
        assert_eq!(
            selector,
            UsbSelector::VidPidWithSelector {
                vid: 0x1FC9,
                pid: 0x0135,
                selector: "HID\\VID_1FC9&PID_0135\\6&7909BF&0&0000".to_owned()
            }
        );
    }

    #[test]
    fn test_parse_vid_pid_with_serial_selector() {
        let selector = parse_usb_identifier("0x1FC9,0x0135,ABCD1234").unwrap();
        assert_eq!(
            selector,
            UsbSelector::VidPidWithSelector {
                vid: 0x1FC9,
                pid: 0x0135,
                selector: "ABCD1234".to_owned()
            }
        );
    }

    #[test]
    fn test_parse_vid_pid_with_topology_selector() {
        let selector = parse_usb_identifier("0x1FC9,0x0135,1-1.4.3").unwrap();
        assert_eq!(
            selector,
            UsbSelector::VidPidWithSelector {
                vid: 0x1FC9,
                pid: 0x0135,
                selector: "1-1.4.3".to_owned()
            }
        );
    }

    #[test]
    fn test_parse_empty_selector_errors() {
        let result = parse_usb_identifier("0x1FC9,0x0135,");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_single_token_non_number_is_path_or_serial() {
        // A single token that isn't a valid VID number is treated as a bare
        // path/serial selector (e.g. a raw path copied from `list-usb`),
        // rather than being rejected as an "invalid VID".
        let selector = parse_usb_identifier("not_a_number").unwrap();
        assert_eq!(
            selector,
            UsbSelector::PathOrSerial {
                selector: "not_a_number".to_owned()
            }
        );
    }

    #[test]
    fn test_parse_windows_path_as_path_or_serial() {
        let path = r"\\?\HID#VID_1FC9&PID_0021#9&1fee2255&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
        let selector = parse_usb_identifier(path).unwrap();
        assert_eq!(
            selector,
            UsbSelector::PathOrSerial {
                selector: path.to_owned()
            }
        );
    }

    #[test]
    fn test_parse_serial_as_path_or_serial() {
        let selector = parse_usb_identifier("ABCD1234").unwrap();
        assert_eq!(
            selector,
            UsbSelector::PathOrSerial {
                selector: "ABCD1234".to_owned()
            }
        );
    }

    #[test]
    fn test_parse_bare_topology_path_as_path_or_serial() {
        // A bare "1-1.4.3" topology path (no vid/pid prefix) is parsed as a
        // generic PathOrSerial selector, same as any other bare path/serial;
        // the topology match itself happens later in `device_matches_selector`.
        let selector = parse_usb_identifier("1-1.4.3").unwrap();
        assert_eq!(
            selector,
            UsbSelector::PathOrSerial {
                selector: "1-1.4.3".to_owned()
            }
        );
    }

    #[test]
    fn test_parse_macos_path_with_colon_as_path_or_serial() {
        // macOS hidapi paths look like "DevSrvsID:4294969627": the leading
        // "DevSrvsID" isn't a valid VID number, so the whole identifier
        // (including the ':' and everything after it) must be kept intact as
        // a single bare selector, rather than being mis-split into a bogus
        // "<vid>,<pid>" pair (which would previously fail with "Invalid VID:
        // DevSrvsID").
        let path = "DevSrvsID:4294969627";
        let selector = parse_usb_identifier(path).unwrap();
        assert_eq!(
            selector,
            UsbSelector::PathOrSerial {
                selector: path.to_owned()
            }
        );
    }

    #[test]
    fn test_parse_empty_identifier_errors() {
        let result = parse_usb_identifier("");
        assert!(result.is_err());
    }

    #[test]
    fn test_nxp_vid_filter_accepts_known_nxp_vids() {
        for vid in NXP_USB_VIDS {
            assert!(is_nxp_vid(vid));
        }
    }

    #[test]
    fn test_nxp_vid_filter_rejects_unknown_vendor() {
        // Common non-NXP VIDs (mouse/keyboard vendors) must not be mistaken for NXP devices.
        for vid in [0x413Cu16, 0x046Du16, 0x8087u16] {
            assert!(!is_nxp_vid(vid));
        }
    }

    #[test]
    fn test_usb_arg_round_trips_through_parse_usb_identifier() {
        let device = UsbDeviceInfo {
            vid: 0x1FC9,
            pid: 0x0021,
            path: r"\\?\HID#VID_1FC9&PID_0021#9&1fee2255&0&0000#{...}".to_owned(),
            topology_path: None,
            serial_number: None,
            product_string: Some("USB COMPOSITE DEVICE".to_owned()),
            manufacturer_string: None,
        };
        let arg = device.usb_arg();
        assert_eq!(arg, r"0x1FC9,0x0021,\\?\HID#VID_1FC9&PID_0021#9&1fee2255&0&0000#{...}");

        // The printed argument must be directly usable as a `-u/--usb` value.
        let selector = parse_usb_identifier(&arg).unwrap();
        assert_eq!(
            selector,
            UsbSelector::VidPidWithSelector {
                vid: 0x1FC9,
                pid: 0x0021,
                selector: device.path,
            }
        );
    }

    #[test]
    fn test_usb_topology_arg_none_without_topology_path() {
        let device = UsbDeviceInfo {
            vid: 0x1FC9,
            pid: 0x0021,
            path: "/dev/hidraw3".to_owned(),
            topology_path: None,
            serial_number: None,
            product_string: None,
            manufacturer_string: None,
        };
        assert_eq!(device.usb_topology_arg(), None);
    }

    #[test]
    fn test_usb_topology_arg_formats_like_usb_arg() {
        let device = UsbDeviceInfo {
            vid: 0x1FC9,
            pid: 0x0021,
            path: "/dev/hidraw3".to_owned(),
            topology_path: Some("1-1.4.3".to_owned()),
            serial_number: None,
            product_string: None,
            manufacturer_string: None,
        };
        let arg = device.usb_topology_arg().unwrap();
        assert_eq!(arg, "0x1FC9,0x0021,1-1.4.3");

        // Must round-trip through the parser, same as `usb_arg`.
        let selector = parse_usb_identifier(&arg).unwrap();
        assert_eq!(
            selector,
            UsbSelector::VidPidWithSelector {
                vid: 0x1FC9,
                pid: 0x0021,
                selector: "1-1.4.3".to_owned(),
            }
        );
    }

    #[test]
    fn test_topology_path_from_location_id() {
        assert_eq!(topology_path_from_location_id(0x1432_0000).as_deref(), Some("20-3.2"));
        assert_eq!(topology_path_from_location_id(0x0110_0000).as_deref(), Some("1-1"));
        assert_eq!(
            topology_path_from_location_id(0x8012_3456).as_deref(),
            Some("128-1.2.3.4.5.6")
        );
        // Ports above 9 are still single hex digits in the locationID.
        assert_eq!(topology_path_from_location_id(0x02FA_0000).as_deref(), Some("2-15.10"));
        // A zero digit terminates the chain; anything after it is ignored.
        assert_eq!(topology_path_from_location_id(0x0130_5000).as_deref(), Some("1-3"));
        // Root hubs / non-USB devices have no port component.
        assert_eq!(topology_path_from_location_id(0), None);
        assert_eq!(topology_path_from_location_id(0x8000_0000), None);
    }

    #[test]
    fn test_parse_bare_macos_topology_path_as_path_or_serial() {
        assert_eq!(
            parse_usb_identifier("20-3.2").unwrap(),
            UsbSelector::PathOrSerial {
                selector: "20-3.2".to_owned()
            }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_is_usb_topology_name_accepts_valid_names() {
        for name in ["1-1", "1-1.4", "1-1.4.3", "2-10.3.1.4"] {
            assert!(is_usb_topology_name(name), "{name} should be a valid topology name");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_is_usb_topology_name_rejects_non_topology_names() {
        for name in [
            "usb1",        // root hub directory, not a topology name
            "hidraw0",     // hidraw device node name
            "1-1.4.3:1.0", // USB interface directory (has a ':')
            "1-",          // no port component
            "-1",          // no bus component
            "1-1.",        // trailing empty port component
            "1-.1",        // leading empty port component
            "a-1",         // non-numeric bus
            "1-1.a",       // non-numeric port
            "",
        ] {
            assert!(
                !is_usb_topology_name(name),
                "{name} should not be a valid topology name"
            );
        }
    }
}
