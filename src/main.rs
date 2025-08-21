// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#![allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    reason = "Docs here are not used by rustdoc, they are used by clap for CLI help"
)]

use std::{
    fs::File,
    io::{Read, Write},
};
mod parsers;

use clap::{Arg, ArgGroup, Parser, Subcommand};
use log::{LevelFilter, debug, warn};
use mboot::{
    CommunicationError, GetPropertyResponse, KeyProvisioningResponse, McuBoot, ReadMemoryResponse,
    protocols::{Protocol, ProtocolOpen, i2c::I2CProtocol, uart::UARTProtocol, usb::USBProtocol},
    tags::{
        command::{KeyProvOperation, TrustProvOperation},
        property::PropertyTagDiscriminants,
        status::StatusCode,
    },
};
use pretty_hex::{HexConfig, PrettyHex};

fn main() -> anyhow::Result<()> {
    let args = std::env::args();
    // FIXME this probably isn't the best solution to ignore "--", but it's the best I've come up with to stay compatible with the python version
    let args = Args::parse_from(args.filter(|arg| arg != "--"));
    env_logger::builder()
        .filter_level(match args.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        })
        .format_timestamp_millis()
        .parse_default_env()
        .init();

    // clap ensures, that at least one of the device is Some
    if args.device.port.is_some() {
        let mut blhost = Blhost::new_from_uart(args)?;
        run_blhost(&mut blhost)?;
    } else if args.device.i2c.is_some() {
        let mut blhost = Blhost::new_from_i2c(args)?;
        run_blhost(&mut blhost)?;
    } else if args.device.usb.is_some() {
        let mut blhost = Blhost::new_from_usb(args)?;
        run_blhost(&mut blhost)?;
    }
    Ok(())
}

fn run_blhost<T>(blhost: &mut Blhost<T>) -> anyhow::Result<()>
where
    T: Protocol,
{
    blhost.execute()?;
    Ok(())
}

// TODO the original blhost can just *recover* the board when the program crashes and doesn't send ACK? would be nice to have that here too

#[derive(clap::Args, Debug)]
#[group(required = true, multiple = false)]
struct Device {
    /// I2C device identifier in format /dev/i2c-X[:0xYY] where X is the bus number
    /// and YY is the optional slave address [default: 0x10]
    #[arg(long)]
    i2c: Option<String>,
    /// UART port identifier
    ///
    /// Baudrate can be optionally specified after a colon, e.g. "COM1,38400".
    /// Default baudrate is 57600.
    #[arg(long, short)]
    port: Option<String>,
    /// USB-HID device identifier in format "vid,pid" (e.g., "0x1FC9,0x0135")
    #[arg(long, short)]
    usb: Option<String>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(flatten)]
    device: Device,

    /// Serial read timeout in milliseconds
    #[arg(short, long, default_value_t = 5000)]
    timeout: u64,

    /// Polling interval for reading in milliseconds
    #[arg(long, default_value_t = 1)]
    polling_interval: u64,

    /// Surpress status response and response words
    #[arg(short, long)]
    silent: bool,
    /// Verbosity level, use more for more verbosity
    ///
    /// -v means info, -vv means debug and -vvv and more is trace level. If RUST_LOG environment
    /// variable is set, it overrides this option. For more documentation about it, refer to
    /// env_logger crate.
    #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 0)]
    verbose: u8,
    /// Command to send to device
    #[command(subcommand)]
    command: Commands,
    #[arg(long, hide = true)]
    secret: bool,
}

// this can't be CommandTag directly, some commands (like ReadMemory) provide additional options
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Queries various bootloader properties and settings.
    GetProperty {
        /// Number or name representing the requested property
        ///
        /// Available properties:
        ///  1 or 'current-version'             Bootloader version
        ///  2 or 'available-peripherals'       Available peripherals
        ///  3 or 'flash-start-address'         Start of program flash, <index> is required
        ///  4 or 'flash-size'                  Size of program flash, <index> is required
        ///  5 or 'flash-sector-size'           Size of flash sector, <index> is required
        ///  6 or 'flash-block-count'           Blocks in flash array, <index> is required
        ///  7 or 'available-commands'          Available commands
        ///  8 or 'check-status'                Check Status, <status id> is required
        ///  9 or 'reserved'
        /// 10 or 'verify-writes'               Verify Writes flag
        /// 11 or 'max-packet-size'             Max supported packet size
        /// 12 or 'reserved-regions'            Reserved regions
        /// 13 or 'reserved'
        /// 14 or 'ram-start-address'           Start of RAM, <index> is required
        /// 15 or 'ram-size-in-bytes'           Size of RAM, <index> is required
        /// 16 or 'system-device-id'            System device identification
        /// 17 or 'security-state'              Flash security state
        /// 18 or 'unique-device-id'            Unique device identification
        /// 19 or 'flash-fac-support'           FAC support flag
        /// 20 or 'flash-access-segment-size'   FAC segment size
        /// 21 or 'flash-access-segment-count'  FAC segment count
        /// 22 or 'flash-read-margin'           Read margin level of program flash
        /// 23 or 'qspi/otfad-init-status'      QuadSpi initialization status
        /// 24 or 'target-version'              Target version
        /// 25 or 'external-memory-attributes'  External memory attributes, <memoryId> is required
        /// 26 or 'reliable-update-status'      Reliable update status
        /// 27 or 'flash-page-size'             Flash page size, <index> is required
        /// 28 or 'irq-notifier-pin'            Interrupt notifier pin
        /// 29 or 'pfr-keystore_update-opt'     PFR key store update option
        /// 30 or 'byte-write-timeout-ms'       Byte write timeout in ms
        /// 31 or 'fuse-locked-status'          Fuse Locked Status
        ///
        /// for kw45xx/k32w1xx devices:
        /// 10 or 'verify-erases'               Verify Erases flag
        /// 20 or 'boot status'                 Value of Boot Status Register
        /// 21 or 'loadable-fw-version'         LoadableFWVersion
        /// 22 or 'fuse-program-voltage'        Fuse Program Voltage
        ///
        /// for mcxa1xx devices:
        /// 17 or 'life-cycle'                  Life Cycle
        ///
        /// Note: Not all the properties are available for all devices.
        // a value parser from clap could be used here; however, it can't convert from repr
        #[arg(value_parser=PropertyTagDiscriminants::parse_property, verbatim_doc_comment)]
        property_tag: PropertyTagDiscriminants,
        /// ID of the memory
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t=0)]
        memory_index: u32,
    },
    /// Reset the device.
    ///
    /// Response packet is sent before the device resets.
    Reset,
    /// Jumps to code at the provided address.
    ///
    /// The system is returned to a reset state before the jump.
    Execute {
        /// Jump address.
        #[arg(value_parser=parsers::parse_number::<u32>)]
        start_address: u32,
        /// Function argument pointer passed to R0.
        #[arg(value_parser=parsers::parse_number::<u32>)]
        argument: u32,
        /// Stack pointer. If set to zero, the code being called should
        /// set the stack pointer before using the stack.
        #[arg(value_parser=parsers::parse_number::<u32>)]
        stackpointer: u32,
    },
    /// Invokes code at an address, passing an argument to it.
    ///
    Call {
        /// Jump address.
        #[arg(value_parser=parsers::parse_number::<u32>)]
        start_address: u32,
        /// Function argument pointer passed to R0.
        #[arg(value_parser=parsers::parse_number::<u32>)]
        argument: u32,
    },
    /// Perform an erase of the entire flash memory.
    ///
    /// Note: Protected regions are excluded.
    FlashEraseAll {
        /// ID of the memory to erase
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t=0)]
        memory_id: u32,
    },
    /// Fills the memory with a pattern.
    FillMemory {
        /// Starting address
        #[arg(value_parser=parsers::parse_number::<u32>)]
        start_address: u32,
        /// Number of bytes to fill
        #[arg(value_parser=parsers::parse_number::<u32>)]
        byte_count: u32,
        /// Pattern to fill
        #[arg(value_parser=parsers::parse_number::<u32>)]
        pattern: u32,
    },
    /// Reads the memory and writes it to a file or stdout.
    ReadMemory {
        /// Starting address
        #[arg(value_parser=parsers::parse_number::<u32>)]
        start_address: u32,
        /// Number of bytes to read
        #[arg(value_parser=parsers::parse_number::<u32>)]
        byte_count: u32,
        /// Store read bytes into <FILE>
        ///
        /// If you need to specify [MEMORY_ID], use '-' instead of filename to print to stdout.
        file: Option<String>,
        /// ID of the memory to read from
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t=0)]
        memory_id: u32,
        /// Use hexdump format
        #[arg(long, short, default_value_t = false)]
        use_hexdump: bool,
    },
    /// Changes properties and options in the bootloader.
    ///
    /// Accepts the same <PROPERTY_TAG> used with the get-property sub-command.
    SetProperty {
        /// Number or name representing the requested property
        ///
        /// Available properties to set:
        /// 10 or 'verify-writes'               Verify Writes flag
        /// 22 or 'flash-read-margin'           Read margin level of program flash
        /// 28 or 'irq-notify-pin'              Interrupt notifier pin
        /// 29 or 'pfr-keystore_update-opt'     PFR key store update option
        /// 30 or 'byte-write-timeout-ms'       Byte write timeout in ms
        ///
        /// for kw45xx/k32w1xx devices:
        /// 10 or 'verify-erases'               Verify Erases flag
        /// 22 or 'fuse-program-voltage'        Fuse Program Voltage
        ///
        /// Note: Not all properties can be set on all devices.
        #[arg(value_parser=PropertyTagDiscriminants::parse_property, verbatim_doc_comment)]
        property_tag: PropertyTagDiscriminants,
        /// Value to set <PROPERTY_TAG> to
        #[arg(value_parser=parsers::parse_number::<u32>)]
        value: u32,
    },
    /// Sets a config at internal memory to memory with ID.
    ///
    /// The specified configuration block must have been previously written to memory using the write-memory command.
    ConfigureMemory {
        /// ID of the memory
        #[arg(value_parser=parsers::parse_number::<u32>)]
        memory_id: u32,
        /// Starting address
        #[arg(value_parser=parsers::parse_number::<u32>)]
        address: u32,
    },
    /// Erase Complete Flash and Unlock.
    FlashEraseAllUnsecure,
    /// Erases one or more sectors of the flash memory.
    ///
    /// The start <ADDRESS> and <BYTE_COUNT> must be a multiple of the word size.
    /// The entire sector(s) containing the start and end address is erased.
    FlashEraseRegion {
        /// Starting address
        #[arg(value_parser=parsers::parse_number::<u32>)]
        start_address: u32,
        /// Number of bytes to erase
        #[arg(value_parser=parsers::parse_number::<u32>)]
        byte_count: u32,
        /// ID of the memory to erase
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t=0)]
        memory_id: u32,
    },
    /// Write memory from a file or CLI.
    ///
    /// Only one of <FILE> (with <LIMIT>) or <BYTES> must be specified.
    #[command(
        override_usage = concat!(color_print::cstr!("<bold>rblhost write-memory"), " <START_ADDRESS> FILE[,LIMIT] | {{HEX_DATA}} [MEMORY_ID]"),
        args=[
            Arg::new("FILE").help("write the content of this file"),
            Arg::new("LIMIT").help("If specified, load only first [LIMIT] bytes from FILE"),
            Arg::new("HEX_DATA").help("A string of hex values: {{112233}}, {{11 22 33}}"),
        ]
    )]
    WriteMemory {
        /// Starting address
        #[arg(value_parser=parsers::parse_number::<u32>, display_order=0)]
        start_address: u32,
        #[arg(value_parser=parsers::parse_hex_values, hide = true)]
        bytes: Box<[u8]>,
        /// ID of the memory to write
        #[arg(default_value_t = 0)]
        memory_id: u32,
    },
    /// Program fuse.
    ///
    /// Only one of <FILE> (with optional <BYTE_COUNT>) or <HEX_DATA> must be specified.
    #[command(
    override_usage = concat!(
        color_print::cstr!("<bold>rblhost fuse-program"),
        " <START_ADDRESS> FILE[,BYTE_COUNT] | {{HEX_DATA}} [MEMORY_ID]"
    ),
    group = ArgGroup::new("file_input").args(&["file", "byte_count"]),
    group = ArgGroup::new("hex_input").args(&["hex_data"]),
    group = ArgGroup::new("input")
        .args(&["file", "hex_data"])
        .required(true)
        .multiple(false)
)]
    FuseProgram {
        /// Start address.
        #[arg(value_parser = parsers::parse_number::<u32>, display_order = 0)]
        start_address: u32,

        /// Write the content of this file.
        file: Option<String>,

        /// If specified, load only first BYTE_COUNT number of bytes.
        #[arg(requires = "file")]
        byte_count: Option<u32>,

        /// A string of hex values: {{112233}}, {{11 22 33}}
        #[arg(value_parser = parsers::parse_hex_values)]
        hex_data: Option<Box<[u8]>>,

        /// ID of memory to read from (default: 0)
        #[arg(default_value_t = 0)]
        memory_id: u32,
    },
    /// Reads the fuse and writes it to the file or stdout.
    FuseRead {
        /// Start address.
        #[arg(value_parser=parsers::parse_number::<u32>)]
        start_address: u32,
        /// Number of bytes to read.
        #[arg(value_parser=parsers::parse_number::<u32>)]
        byte_count: u32,
        /// Store result into this file, if not specified use stdout.
        file: Option<String>,
        /// ID of memory to read from (default: 0)
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t=0)]
        memory_id: u32,
        /// Use hexdump format
        #[arg(long, short, default_value_t = false)]
        use_hexdump: bool,
    },
    /// Receives a file in a Secure Binary (SB) format.
    ReceiveSbFile {
        #[arg(value_parser=|s: &str| parsers::parse_file(s, None))]
        bytes: Box<[u8]>,
    },

    /// Read from MCU flash program once region (eFuse/OTP)
    FlashReadOnce {
        /// Start index of the eFuse/OTP region
        #[arg(value_parser=parsers::parse_number::<u32>)]
        index: u32,

        /// Number of bytes to read (default: 4)
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t=4)]
        count: u32,
    },

    /// Write into MCU program once region (eFuse/OTP)
    FlashProgramOnce {
        /// Start index of the eFuse/OTP region
        #[arg(value_parser=parsers::parse_number::<u32>)]
        index: u32,

        /// Value to write (32-bit)
        #[arg(value_parser=parsers::parse_number::<u32>)]
        data: u32,

        /// Number of bytes to write (default: 4)
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t=4)]
        count: u32,

        /// Verify that data were written correctly
        #[arg(long, default_value_t = false)]
        verify: bool,
    },

    /// Group of subcommands related to trust provisioning
    #[command(subcommand)]
    TrustProvisioning(TrustProvOperation),
    /// Group of subcommands related to key provisioning
    #[command(subcommand)]
    KeyProvisioning(KeyProvOperation),
    /// Sends a boot image file to the device.
    ///
    /// Only binary files are supported. The <FILE> must be a bootable
    /// image which contains the boot image header supported by the MCU
    /// bootloader.
    LoadImage {
        /// Boot file to load
        file: String,
    },
}

pub struct Blhost<T>
where
    T: Protocol,
{
    args: Args,
    boot: McuBoot<T>,
}

const DEFAULT_BAUDRATE: u32 = 57600;
impl Blhost<UARTProtocol> {
    fn new_from_uart(args: Args) -> Result<Self, CommunicationError> {
        let port_spec = args
            .device
            .port
            .as_ref()
            .expect("open_uart called without UART argument");
        // Parse port and optional baudrate
        let mut parts = port_spec.split(',');
        let port_name = parts.next().unwrap();
        let baudrate = parts
            .next()
            .map_or(DEFAULT_BAUDRATE, |v| v.parse().unwrap_or(DEFAULT_BAUDRATE));

        // Use UART protocol with specified baudrate and timeout
        let boot = McuBoot::new(UARTProtocol::open_with_options(
            port_name,
            baudrate,
            std::time::Duration::from_millis(args.timeout),
            std::time::Duration::from_millis(args.polling_interval),
        )?);
        Ok(Blhost { args, boot })
    }
}

impl Blhost<I2CProtocol> {
    fn new_from_i2c(args: Args) -> Result<Self, CommunicationError> {
        let i2c_device = args
            .device
            .i2c
            .as_ref()
            .expect("new_from_i2c called without I2C argument");
        let boot = McuBoot::new(I2CProtocol::open(i2c_device)?);
        Ok(Blhost { args, boot })
    }
}

impl Blhost<USBProtocol> {
    fn new_from_usb(args: Args) -> Result<Self, CommunicationError> {
        let usb_device = args
            .device
            .usb
            .as_ref()
            .expect("new_from_usb called without USB argument");
        let boot = McuBoot::new(USBProtocol::open_with_options(
            usb_device,
            0, // Baudrate not used for USB
            std::time::Duration::from_millis(args.timeout),
            std::time::Duration::from_millis(args.polling_interval),
        )?);
        Ok(Blhost { args, boot })
    }
}

impl<T> Blhost<T>
where
    T: Protocol,
{
    pub fn new(args: Args, device: T) -> Blhost<T> {
        Blhost {
            args,
            boot: McuBoot::new(device),
        }
    }

    #[allow(clippy::too_many_lines, reason = "match statement here will always be long")]
    pub fn execute(&mut self) -> Result<(), CommunicationError> {
        self.boot.progress_bar = !self.args.silent;

        match self.args.command {
            Commands::GetProperty {
                property_tag,
                memory_index,
            } => {
                let response = &self.boot.get_property(property_tag, memory_index)?;
                self.display_property(response);
            }
            Commands::Reset => {
                let status = self.boot.reset()?;
                self.display_status(status);
            }
            Commands::Execute {
                start_address,
                argument,
                stackpointer,
            } => {
                let status = self.boot.execute(start_address, argument, stackpointer)?;
                self.display_status(status);
            }
            Commands::Call {
                start_address,
                argument,
            } => {
                let status = self.boot.call(start_address, argument)?;
                self.display_status(status);
            }
            Commands::FlashEraseAll { memory_id } => {
                let status = self.boot.flash_erase_all(memory_id)?;
                self.display_status(status);
            }
            Commands::FillMemory {
                start_address,
                byte_count,
                pattern,
            } => {
                let status = self.boot.fill_memory(start_address, byte_count, pattern)?;
                self.display_status(status);
            }
            Commands::ReadMemory {
                start_address,
                byte_count,
                ref file,
                memory_id,
                use_hexdump,
            } => match file.as_deref() {
                None | Some("-") => {
                    let response = self.boot.read_memory(start_address, byte_count, memory_id)?;
                    self.display_memory_bytes(&response, byte_count, use_hexdump);
                }
                Some(file_name) => {
                    let response = self.boot.read_memory(start_address, byte_count, memory_id)?;
                    let mut file = File::create(file_name).map_err(CommunicationError::FileError)?;
                    file.write_all(&response.bytes)?;
                    self.display_memory(&response, byte_count);
                }
            },
            Commands::SetProperty { property_tag, value } => {
                let status = self.boot.set_property(property_tag, value)?;
                self.display_status(status);
            }
            Commands::ConfigureMemory { memory_id, address } => {
                let status = self.boot.configure_memory(memory_id, address)?;
                self.display_status(status);
            }
            Commands::FlashEraseAllUnsecure => {
                let status = self.boot.flash_erase_all_unsecure()?;
                self.display_status(status);
            }
            Commands::FlashEraseRegion {
                start_address,
                byte_count,
                memory_id,
            } => {
                let status = self.boot.flash_erase_region(start_address, byte_count, memory_id)?;
                self.display_status(status);
            }
            Commands::WriteMemory {
                start_address,
                ref bytes,
                memory_id,
            } => {
                let status = self.boot.write_memory(start_address, memory_id, bytes)?;
                self.display_status(status);
            }
            Commands::ReceiveSbFile { ref bytes } => {
                let status = self.boot.receive_sb_file(bytes)?;
                self.display_status(status);
            }
            Commands::TrustProvisioning(ref operation) => {
                let (status, data) = self.boot.trust_provisioning(operation)?;
                self.display_status_words(status, &data);
                self.display_trust_prov(operation, &data);
            }
            Commands::KeyProvisioning(ref operation) => match operation {
                KeyProvOperation::SetUserKey { key_type, key_data } => {
                    if !self.args.silent {
                        debug!(
                            "Setting user key of type {} with {} bytes of data",
                            key_type,
                            key_data.len()
                        );
                    }
                    let response = self.boot.key_provisioning(operation)?;
                    match response {
                        KeyProvisioningResponse::KeyStore { status, .. } | KeyProvisioningResponse::Status(status) => {
                            self.display_status(status);
                        }
                    }
                }
                KeyProvOperation::SetKey { key_type, key_size } => {
                    if !self.args.silent {
                        debug!("Generating intrinsic key of type {key_type} with size {key_size} bytes");
                    }
                    let response = self.boot.key_provisioning(operation)?;
                    match response {
                        KeyProvisioningResponse::KeyStore { status, .. } | KeyProvisioningResponse::Status(status) => {
                            self.display_status(status);
                        }
                    }
                }
                KeyProvOperation::ReadKeyStore { file, use_hexdump } => {
                    debug!("Reading key store from device");

                    // Execute the key provisioning command
                    let response = self.boot.key_provisioning(operation)?;

                    match response {
                        KeyProvisioningResponse::KeyStore {
                            status,
                            response_words,
                            bytes,
                        } => {
                            if status.is_success() {
                                // Write to file
                                let mut output_file = File::create(file).map_err(CommunicationError::FileError)?;
                                output_file.write_all(&bytes)?;

                                if !self.args.silent {
                                    println!("Successfully wrote {} bytes to file: {}", bytes.len(), file);

                                    if *use_hexdump {
                                        // Display the data in hexdump format
                                        let cfg = HexConfig {
                                            title: false,
                                            group: 8,
                                            width: 16,
                                            ascii: true,
                                            ..HexConfig::default()
                                        };
                                        println!("{:?}", bytes.hex_conf(cfg));
                                    }
                                }

                                self.display_status_words(status, &response_words);
                            } else {
                                self.display_status(status);
                            }
                        }
                        KeyProvisioningResponse::Status(status) => {
                            self.display_status(status);
                        }
                    }
                }
                _ => {
                    let response = self.boot.key_provisioning(operation)?;
                    match response {
                        KeyProvisioningResponse::KeyStore { status, .. } | KeyProvisioningResponse::Status(status) => {
                            self.display_status(status);
                        }
                    }
                }
            },
            Commands::FlashReadOnce { index, count } => {
                let value = self.boot.flash_read_once(index, count)?;
                if !self.args.silent {
                    println!("Read value: {value} (0x{value:X})");
                }
            }
            Commands::FlashProgramOnce {
                index,
                data,
                count,
                verify,
            } => {
                let status = self.boot.flash_program_once(index, count, data, verify)?;
                self.display_status(status);

                if status == StatusCode::OtpVerifyFail {
                    warn!("Verification failed - written value doesn't match read value");
                }
            }
            Commands::FuseRead {
                start_address,
                byte_count,
                ref file,
                memory_id,
                use_hexdump,
            } => match file.as_deref() {
                None | Some("-") => {
                    let response = self.boot.fuse_read(start_address, byte_count, memory_id)?;
                    self.display_memory_bytes(&response, byte_count, use_hexdump);
                }
                Some(file_name) => {
                    let response = self.boot.fuse_read(start_address, byte_count, memory_id)?;
                    let mut file = File::create(file_name).map_err(CommunicationError::FileError)?;
                    file.write_all(&response.bytes)?;
                    self.display_memory(&response, byte_count);
                }
            },
            Commands::FuseProgram {
                start_address,
                ref file,
                byte_count,
                ref hex_data,
                memory_id,
            } => {
                let bytes: Vec<u8> = if let Some(hex) = hex_data {
                    hex.to_vec()
                } else if let Some(file_path) = file {
                    let mut file = File::open(file_path).map_err(CommunicationError::FileError)?;
                    let mut buffer = Vec::new();
                    match byte_count {
                        Some(limit) => {
                            buffer.resize(limit as usize, 0);
                            file.read_exact(&mut buffer).map_err(CommunicationError::FileError)?;
                        }
                        None => {
                            file.read_to_end(&mut buffer).map_err(CommunicationError::FileError)?;
                        }
                    }
                    buffer
                } else {
                    return Err(CommunicationError::InvalidData);
                };
                let status = self.boot.fuse_program(start_address, memory_id, &bytes)?;
                self.display_status(status);
            }
            Commands::LoadImage { ref file } => {
                let mut file = File::open(file).map_err(CommunicationError::FileError)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).map_err(CommunicationError::FileError)?;
                let status = self.boot.load_image(&buffer)?;
                self.display_status(status);
            }
        }

        if self.args.secret {
            println!("congratulations! you found the secret 🍨");
        }

        Ok(())
    }

    fn display_memory_bytes(&self, response: &ReadMemoryResponse, byte_count: u32, use_hexdump: bool) {
        if use_hexdump {
            let cfg = HexConfig {
                title: false,
                group: 8,
                width: 16,
                ascii: true,
                ..HexConfig::default()
            };
            println!("{:?}", response.bytes.hex_conf(cfg));
        } else {
            for byte_line in response.bytes.chunks(16) {
                for byte in byte_line {
                    print!("{byte:02X?} ");
                }
                println!();
            }
        }
        self.display_memory(response, byte_count);
    }

    fn display_memory(&self, response: &ReadMemoryResponse, byte_count: u32) {
        self.display_status_words(response.status, &response.response_words);
        if !self.args.silent {
            println!("Read {} of {byte_count} bytes.", response.bytes.len());
        }
    }

    fn display_property(&self, response: &GetPropertyResponse) {
        self.display_status_words(response.status, &response.response_words);
        println!("{}", response.property);
    }

    fn display_status_words(&self, status: StatusCode, response_words: &[u32]) {
        self.display_status(status);
        self.display_words(response_words);
    }

    fn display_words(&self, response_words: &[u32]) {
        if !self.args.silent {
            for (i, word) in response_words.iter().enumerate() {
                let i = i + 1;
                println!("Response word {i} = {word} ({word:#x})");
            }
        }
    }

    fn display_status(&self, status: StatusCode) {
        if !self.args.silent {
            println!("Response status = {0} ({0:#x}) {1}.", u32::from(status), status);
        }
    }

    fn display_trust_prov(&self, operation: &TrustProvOperation, response: &[u32]) {
        if !self.args.silent {
            println!("Output data size/value(s) is (are):");
            match operation {
                TrustProvOperation::OemGenMasterShare { .. } => println!(
                    "\
                    \tOEM Share size: {0} ({0:#02X})\n\
                    \tOEM Master Share size: {1} ({1:#02X})\n\
                    \tCust Cert Puk size: {2} ({2:#02X})",
                    response[0], response[1], response[2]
                ),
                TrustProvOperation::OemSetMasterShare { .. } => {}
            }
        }
    }
}
