// Copyright 2025-2026 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#![allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    reason = "Docs here are not used by rustdoc, they are used by clap for CLI help"
)]

use std::{
    ffi::OsString,
    fmt::Write as FmtWrite,
    fs::File,
    io::{Read, Write},
};
mod batch;
mod image_parser;
mod parsers;
use std::path::Path;

use clap::{Arg, ArgGroup, CommandFactory, FromArgMatches, Parser, Subcommand};
use image_parser::ImageParser;
use log::{LevelFilter, debug, warn};
use mboot::{
    CommunicationError, EFUSE_LOCK_FLAG, GetPropertyResponse, KeyProvisioningResponse, McuBoot, ReadMemoryResponse,
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
    let args = match parse_args_from(args.filter(|arg| arg != "--")) {
        Ok(args) => args,
        Err(error) => error.exit(),
    };
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

    // `list-usb` does not operate on a specific device, so it is handled before the
    // device group (--port/--i2c/--usb) is required to be set.
    if let Commands::ListUsb { all } = args.command {
        return list_usb(all);
    }

    if args.device.port.is_some() {
        let mut blhost = Blhost::new_from_uart(args)?;
        run_blhost(&mut blhost)?;
    } else if args.device.i2c.is_some() {
        let mut blhost = Blhost::new_from_i2c(args)?;
        run_blhost(&mut blhost)?;
    } else if args.device.usb.is_some() {
        let mut blhost = Blhost::new_from_usb(args)?;
        run_blhost(&mut blhost)?;
    } else {
        anyhow::bail!(
            "No device specified. Use one of --port, --i2c, or --usb \
             (run 'rblhost list-usb' to see connected USB-HID devices)."
        );
    }
    Ok(())
}

fn parse_args_from<I, T>(args: I) -> Result<Args, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = cli_command().try_get_matches_from(args)?;
    Args::from_arg_matches(&matches)
}

fn cli_command() -> clap::Command {
    let mut command = Args::command();
    configure_help_flags(&mut command);
    command
}

pub(crate) fn configure_help_flags(command: &mut clap::Command) {
    for subcommand in command.get_subcommands_mut() {
        match subcommand.get_name() {
            "read-memory" | "fuse-read" | "read_key_store" => add_long_help_only(subcommand),
            _ => {}
        }
        configure_help_flags(subcommand);
    }
}

fn add_long_help_only(command: &mut clap::Command) {
    *command = command.clone().disable_help_flag(true).arg(
        Arg::new("help")
            .long("help")
            .action(clap::ArgAction::Help)
            .help("Print help"),
    );
}

/// Lists currently connected USB-HID devices (VID/PID, OS path, serial number).
///
/// By default, only devices with a known NXP vendor ID are shown. For each device, prints
/// a ready-to-use `-u/--usb` argument that can be copy-pasted directly, e.g. into
/// `rblhost -u "<value>" get-property 1`.
fn list_usb(all: bool) -> anyhow::Result<()> {
    let devices = USBProtocol::list_devices(all)?;
    if devices.is_empty() {
        if all {
            println!("No USB-HID devices found.");
        } else {
            println!("No NXP USB-HID devices found. Use 'list-usb --all' to see all connected HID devices.");
        }
    } else {
        for (index, device) in devices.iter().enumerate() {
            if index > 0 {
                println!();
            }
            println!("USB device #{}:", index + 1);
            println!("  VID/PID:  0x{:04X}:0x{:04X}", device.vid, device.pid);
            println!("  Path:     {}", device.path);
            if let Some(topology) = device.topology_path.as_deref() {
                println!("  Topology: {topology}");
            }
            println!(
                "  Serial:   {}",
                device
                    .serial_number
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("N/A")
            );
            println!("  Product:  {}", device.product_string.as_deref().unwrap_or("N/A"));
            println!("  -u \"{}\"", device.usb_arg());
            if let Some(topology_arg) = device.usb_topology_arg() {
                println!("  -u \"{topology_arg}\"  (stable across reconnect/reset, recommended on Linux/macOS)");
            }
        }
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
#[group(multiple = false)]
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
    /// USB-HID device identifier in format "vid[,pid[,selector]]" (e.g., "0x1FC9,0x0135"),
    /// or a bare "<selector>" with no vid/pid at all
    ///
    /// <vid> and <pid> accept decimal or hex (0x-prefixed) numbers. If several devices
    /// share the same VID/PID (e.g. identical boards for parallel programming), add a
    /// third <selector> field: a case-insensitive substring of the device's OS path, its
    /// exact serial number, or its exact USB topology (port-chain) path (Linux/macOS), e.g.
    /// "0x1FC9,0x0135,6&7909BF&0&0000", "0x1FC9,0x0135,ABCD1234", or
    /// "0x1FC9,0x0135,1-1.4.3". Alternatively, if a plain path/serial/topology path is
    /// already unique on its own, it can be used directly without any vid/pid prefix, e.g.
    /// "6&7909BF&0&0000", "ABCD1234", or "1-1.4.3". On Linux and macOS, prefer the topology
    /// path over the OS path: the OS path (/dev/hidrawN, DevSrvsID:<id>) is only unique until
    /// the device is disconnected/reconnected or reset, while the topology path stays stable as long as
    /// the physical USB cabling/hub topology doesn't change. Use 'rblhost list-usb' to see
    /// connected devices and their paths/serial numbers/topology paths.
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

/// Lock option of `efuse-program-once`, matching blhost's `[nolock/lock]` argument.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfuseLock {
    /// Leave the efuse word unlocked
    Nolock,
    /// Permanently lock the efuse word
    Lock,
}

// this can't be CommandTag directly, some commands (like ReadMemory) provide additional options
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Execute commands from a file using one shared device connection.
    ///
    /// One command per line, with shell-style quoting and # comments. Global options
    /// apply to the entire batch. Stops at the first error without reconnecting.
    Batch {
        /// Path to the command file
        command_file: String,
    },

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
        #[arg(long = "use-hexdump", short = 'h', default_value_t = false)]
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
        #[arg(long = "use-hexdump", short = 'h', default_value_t = false)]
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

        /// Number of bytes to read (must be 4 or 8, default: 4)
        #[arg(value_parser=parsers::parse_once_byte_count, default_value_t=4)]
        count: u32,
    },

    /// Returns the contents of a specific efuse word.
    ///
    /// Equivalent to `flash-read-once INDEX 4`, matching blhost's `efuse-read-once`.
    EfuseReadOnce {
        /// Address of the OTP word, not the shadowed memory address
        #[arg(value_parser=parsers::parse_number::<u32>)]
        address: u32,
    },

    /// Writes data to a specific efuse word.
    ///
    /// Each efuse bit can only be programmed once. Equivalent to `flash-program-once INDEX 4 DATA`,
    /// matching blhost's `efuse-program-once`.
    EfuseProgramOnce {
        /// Address of the OTP word, not the shadowed memory address
        #[arg(value_parser=parsers::parse_number::<u32>)]
        address: u32,

        /// Data to write, hex digits with or without '0x' prefix (as in blhost)
        #[arg(value_parser=parsers::parse_hex_number::<u32>)]
        data: u32,

        /// Lock the efuse word after programming
        #[arg(value_enum, default_value_t = EfuseLock::Nolock)]
        lock: EfuseLock,

        /// Verify that data were written correctly
        #[arg(long, short, default_value_t = false)]
        verify: bool,
    },

    /// Write into MCU program once region (eFuse/OTP)
    FlashProgramOnce {
        /// Start index of the eFuse/OTP region
        #[arg(value_parser=parsers::parse_number::<u32>)]
        index: u32,

        /// Number of bytes to write (must be 4 or 8)
        #[arg(value_parser=parsers::parse_once_byte_count)]
        count: u32,

        /// Value to write (up to 64-bit, only the low `count` bytes are sent)
        #[arg(value_parser=parsers::parse_number::<u64>)]
        data: u64,

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

    /// Write a formatted image file to flash memory with optional erasing.
    ///
    /// Supports S-record, Intel HEX, Android SPARSE and binary files. The command parses the image file to extract
    /// memory segments and writes them using flash-erase-region and write-memory calls.
    FlashImage {
        /// Path to image file (S-record, Intel HEX, Android SPARSE or binary)
        file: String,

        /// Erase option: 'erase' to erase flash before writing, 'none' to skip erasing
        #[arg(default_value = "none")]
        erase: String,

        /// ID of memory to write to (default: 0)
        #[arg(value_parser=parsers::parse_number::<u32>, default_value_t = 0)]
        memory_id: u32,
        /// Add this offset to every address in the image
        #[arg(short = 'o', long, value_parser=parsers::parse_number::<u32>, default_value_t = 0)]
        base_offset: u32,
    },

    /// Generates a key blob from a DEK file and writes it to a file.
    GenerateKeyBlob {
        /// Binary file containing the DEK
        #[arg(value_parser = |s: &str| parsers::parse_file(s, None))]
        dek_file: Box<[u8]>,
        /// Output file for the generated binary key blob
        blob_file: String,
        /// BKEK selector: 0/1/2/3 or OPTMK/ZMK/CMK
        #[arg(value_parser = parsers::parse_key_sel, default_value = "0")]
        key_sel: u32,
    },

    /// Lists connected USB-HID devices (VID/PID, OS path, serial number).
    ///
    /// Does not require --port/--i2c/--usb to be specified. Useful to find the <path> or
    /// <serial> of a specific board when several identical devices (same VID/PID) are
    /// connected at once, for use with '-u/--usb <vid>,<pid>,<selector>'. By default, only
    /// devices with a known NXP vendor ID are shown; use --all to see every HID device.
    ListUsb {
        /// Show all HID devices on the system, not just known NXP vendor IDs
        #[arg(long)]
        all: bool,
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

    pub fn execute(&mut self) -> anyhow::Result<()> {
        if let Commands::Batch { ref command_file } = self.args.command {
            let command_file = command_file.clone();
            self.execute_batch(&command_file)
        } else {
            self.execute_command()?;
            Ok(())
        }
    }

    #[allow(clippy::too_many_lines, reason = "match statement here will always be long")]
    fn execute_command(&mut self) -> Result<StatusCode, CommunicationError> {
        self.boot.progress_bar = !self.args.silent;

        let status = match self.args.command {
            Commands::GetProperty {
                property_tag,
                memory_index,
            } => {
                let response = &self.boot.get_property(property_tag, memory_index)?;
                self.display_property(response);
                response.status
            }
            Commands::Reset => {
                let status = self.boot.reset()?;
                self.display_status(status);
                status
            }
            Commands::Execute {
                start_address,
                argument,
                stackpointer,
            } => {
                let status = self.boot.execute(start_address, argument, stackpointer)?;
                self.display_status(status);
                status
            }
            Commands::Call {
                start_address,
                argument,
            } => {
                let status = self.boot.call(start_address, argument)?;
                self.display_status(status);
                status
            }
            Commands::FlashEraseAll { memory_id } => {
                let status = self.boot.flash_erase_all(memory_id)?;
                self.display_status(status);
                status
            }
            Commands::FillMemory {
                start_address,
                byte_count,
                pattern,
            } => {
                let status = self.boot.fill_memory(start_address, byte_count, pattern)?;
                self.display_status(status);
                status
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
                    response.status
                }
                Some(file_name) => {
                    let response = self.boot.read_memory(start_address, byte_count, memory_id)?;
                    let mut file = File::create(file_name).map_err(CommunicationError::FileError)?;
                    file.write_all(&response.bytes)?;
                    self.display_memory(&response, byte_count);
                    response.status
                }
            },
            Commands::SetProperty { property_tag, value } => {
                let status = self.boot.set_property(property_tag, value)?;
                self.display_status(status);
                status
            }
            Commands::ConfigureMemory { memory_id, address } => {
                let status = self.boot.configure_memory(memory_id, address)?;
                self.display_status(status);
                status
            }
            Commands::FlashEraseAllUnsecure => {
                let status = self.boot.flash_erase_all_unsecure()?;
                self.display_status(status);
                status
            }
            Commands::FlashEraseRegion {
                start_address,
                byte_count,
                memory_id,
            } => {
                let status = self.boot.flash_erase_region(start_address, byte_count, memory_id)?;
                self.display_status(status);
                status
            }
            Commands::WriteMemory {
                start_address,
                ref bytes,
                memory_id,
            } => {
                let status = self.boot.write_memory(start_address, memory_id, bytes)?;
                self.display_status(status);
                status
            }
            Commands::ReceiveSbFile { ref bytes } => {
                let status = self.boot.receive_sb_file(bytes)?;
                self.display_status(status);
                status
            }
            Commands::TrustProvisioning(ref operation) => {
                let (status, data) = self.boot.trust_provisioning(operation)?;
                self.display_status_words(status, &data);
                self.display_trust_prov(operation, &data);
                status
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
                            status
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
                            status
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
                            status
                        }
                        KeyProvisioningResponse::Status(status) => {
                            self.display_status(status);
                            status
                        }
                    }
                }
                _ => {
                    let response = self.boot.key_provisioning(operation)?;
                    match response {
                        KeyProvisioningResponse::KeyStore { status, .. } | KeyProvisioningResponse::Status(status) => {
                            self.display_status(status);
                            status
                        }
                    }
                }
            },
            Commands::FlashReadOnce { index, count } => {
                let bytes = self.boot.flash_read_once(index, count)?;
                if !self.args.silent {
                    // Mirror blhost's `flash-read-once` output, which decodes the full
                    // `count`-byte (4 or 8) payload as a single little-endian integer instead of
                    // truncating it to a 32-bit word.
                    let mut buf = [0u8; 8];
                    let len = bytes.len().min(buf.len());
                    buf[..len].copy_from_slice(&bytes[..len]);
                    let value = u64::from_le_bytes(buf);
                    println!("Read value: {value} (0x{value:X})");
                }
                StatusCode::Success
            }
            Commands::FlashProgramOnce {
                index,
                count,
                data,
                verify,
            } => {
                let status = self.boot.flash_program_once(index, count, data, verify)?;
                self.display_status(status);

                if status == StatusCode::OtpVerifyFail {
                    warn!("Verification failed - written value doesn't match read value");
                }
                status
            }
            Commands::EfuseReadOnce { address } => {
                let value = self.boot.efuse_read_once(address)?;
                self.display_status_words(StatusCode::Success, &[4, value]);
                StatusCode::Success
            }
            Commands::EfuseProgramOnce {
                address,
                data,
                lock,
                verify,
            } => {
                let index = match lock {
                    EfuseLock::Lock => address | EFUSE_LOCK_FLAG,
                    EfuseLock::Nolock => address,
                };
                let status = self.boot.efuse_program_once(index, data, verify)?;
                self.display_status(status);

                if status == StatusCode::OtpVerifyFail {
                    warn!("Verification failed - written value doesn't match read value");
                }
                status
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
                    response.status
                }
                Some(file_name) => {
                    let response = self.boot.fuse_read(start_address, byte_count, memory_id)?;
                    let mut file = File::create(file_name).map_err(CommunicationError::FileError)?;
                    file.write_all(&response.bytes)?;
                    self.display_memory(&response, byte_count);
                    response.status
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
                status
            }
            Commands::LoadImage { ref file } => {
                let mut file = File::open(file).map_err(CommunicationError::FileError)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).map_err(CommunicationError::FileError)?;
                let status = self.boot.load_image(&buffer)?;
                self.display_status(status);
                status
            }
            Commands::FlashImage {
                ref file,
                ref erase,
                memory_id,
                base_offset,
            } => {
                let file_path = file.clone();
                let erase_option = erase.clone();
                let status = self.flash_image(&file_path, &erase_option, memory_id, base_offset)?;
                self.display_status(status);
                status
            }
            Commands::GenerateKeyBlob {
                ref dek_file,
                ref blob_file,
                key_sel,
            } => {
                let key_blob = self.boot.generate_key_blob(dek_file, key_sel)?;
                let mut output_file = File::create(blob_file).map_err(CommunicationError::FileError)?;
                output_file.write_all(&key_blob)?;
                let blob_length = u32::try_from(key_blob.len()).map_err(|_| CommunicationError::InvalidData)?;
                self.display_status_words(StatusCode::Success, &[0, blob_length]);
                StatusCode::Success
            }
            Commands::Batch { .. } => unreachable!("Batch is handled in execute()"),
            Commands::ListUsb { .. } => {
                unreachable!("ListUsb is handled in main() before a device connection is opened")
            }
        };

        if self.args.secret {
            println!("congratulations! you found the secret 🍨");
        }

        Ok(status)
    }

    fn display_memory_bytes(&self, response: &ReadMemoryResponse, byte_count: u32, use_hexdump: bool) {
        let formatted = format_memory_bytes(&response.bytes, use_hexdump);
        if !formatted.is_empty() {
            println!("{formatted}");
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
                    \tOEM Share size: {0} ({0:#04X})\n\
                    \tOEM Master Share size: {1} ({1:#04X})\n\
                    \tCust Cert Puk size: {2} ({2:#04X})",
                    response[0], response[1], response[2]
                ),
                TrustProvOperation::OemSetMasterShare { .. } => {}
            }
        }
    }

    fn flash_image(
        &mut self,
        image_file_path: &str,
        erase: &str,
        memory_id: u32,
        base_offset: u32,
    ) -> Result<StatusCode, CommunicationError> {
        const ALIGNMENT: u32 = 1024;

        // Validate file exists
        if !Path::new(image_file_path).is_file() {
            if !self.args.silent {
                println!("Error: The image file does not exist: {image_file_path}");
            }
            return Err(CommunicationError::InvalidData);
        }

        // Validate erase parameter
        if erase != "erase" && erase != "none" {
            if !self.args.silent {
                println!("Error: Invalid erase option '{erase}'. Use 'erase' or 'none'");
            }
            return Err(CommunicationError::InvalidData);
        }

        // Load and parse the image file using the ImageParser
        let mut segments = ImageParser::parse_file(image_file_path)?;
        apply_base_offset(&mut segments, base_offset)?;

        if segments.is_empty() {
            if !self.args.silent {
                println!("Error: No data segments found in image file");
            }
            return Err(CommunicationError::InvalidData);
        }

        if !self.args.silent {
            println!("Loaded image file: {} ({} segments)", image_file_path, segments.len());

            // If it's an S-record file, show additional info
            if ImageParser::is_srec_format(image_file_path)
                && let Ok((_, info)) = ImageParser::parse_srec_with_info(image_file_path)
            {
                if let Some(header) = &info.header {
                    println!("  Header: {header}");
                }
                println!("  Data length: {}", info.data_length);
                println!("  Memory regions: {}", info.memory_regions);
            }

            for (i, segment) in segments.iter().enumerate() {
                println!(
                    "  Segment #{}: address=0x{:08X}, size={} bytes",
                    i + 1,
                    segment.address,
                    segment.data.len()
                );
            }
        }

        // Erase regions if requested
        if erase == "erase" {
            if !self.args.silent {
                println!("Erasing flash regions...");
            }

            for (i, segment) in segments.iter().enumerate() {
                let aligned_start = Self::align_down(segment.address, ALIGNMENT);
                let segment_length = u32::try_from(segment.data.len()).map_err(|_| CommunicationError::InvalidData)?;
                let segment_end = segment
                    .address
                    .checked_add(segment_length)
                    .ok_or(CommunicationError::InvalidData)?;
                let aligned_end = Self::align_up(segment_end, ALIGNMENT).ok_or(CommunicationError::InvalidData)?;
                let aligned_length = aligned_end - aligned_start;

                if !self.args.silent {
                    println!(
                        "Erasing segment #{}: address=0x{:08X}, length=0x{:08X}",
                        i + 1,
                        aligned_start,
                        aligned_length
                    );
                }

                let status = self.boot.flash_erase_region(aligned_start, aligned_length, memory_id)?;
                if !status.is_success() {
                    return Ok(status);
                }
            }
        }

        // Write segments
        for (i, segment) in segments.iter().enumerate() {
            if !self.args.silent {
                println!(
                    "Writing segment #{}: address=0x{:08X}, size={} bytes",
                    i + 1,
                    segment.address,
                    segment.data.len()
                );
            }

            let status = self.boot.write_memory(segment.address, memory_id, &segment.data)?;
            if !status.is_success() {
                return Ok(status);
            }
        }

        Ok(StatusCode::Success)
    }

    // Helper methods for alignment
    fn align_down(value: u32, alignment: u32) -> u32 {
        value & !(alignment - 1)
    }

    fn align_up(value: u32, alignment: u32) -> Option<u32> {
        value.checked_add(alignment - 1).map(|value| value & !(alignment - 1))
    }
}

fn format_memory_bytes(bytes: &[u8], use_hexdump: bool) -> String {
    if use_hexdump {
        let cfg = HexConfig {
            title: false,
            group: 8,
            width: 16,
            ascii: true,
            ..HexConfig::default()
        };
        format!("{:?}", bytes.hex_conf(cfg))
    } else {
        let mut formatted = String::with_capacity(bytes.len().saturating_mul(3));
        for (index, byte) in bytes.iter().enumerate() {
            if index > 0 && index % 16 == 0 {
                formatted.push('\n');
            }
            write!(formatted, "{byte:02x} ").expect("writing to a String cannot fail");
        }
        formatted
    }
}

fn apply_base_offset(segments: &mut [image_parser::ImageSegment], base_offset: u32) -> Result<(), CommunicationError> {
    for segment in segments {
        segment.address = segment
            .address
            .checked_add(base_offset)
            .ok_or(CommunicationError::InvalidData)?;
        let segment_length = u32::try_from(segment.data.len()).map_err(|_| CommunicationError::InvalidData)?;
        segment
            .address
            .checked_add(segment_length)
            .ok_or(CommunicationError::InvalidData)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Args, Commands, EfuseLock, KeyProvOperation, apply_base_offset, format_memory_bytes, parse_args_from};
    use crate::image_parser::ImageSegment;
    use clap::Parser;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_flash_image_base_offset() {
        let mut segments = [
            ImageSegment {
                address: 0x1000,
                data: vec![1],
            },
            ImageSegment {
                address: 0x2000,
                data: vec![2],
            },
        ];
        apply_base_offset(&mut segments, 0x4000).unwrap();
        assert_eq!(segments[0].address, 0x5000);
        assert_eq!(segments[1].address, 0x6000);
    }

    #[test]
    fn test_flash_image_base_offset_rejects_overflow() {
        let mut segments = [ImageSegment {
            address: u32::MAX,
            data: vec![1],
        }];
        assert!(apply_base_offset(&mut segments, 1).is_err());
    }

    #[test]
    fn test_flash_image_accepts_base_offset() {
        let args = parse_args_from([
            "rblhost",
            "--port",
            "COM1",
            "flash-image",
            "image.srec",
            "--base-offset",
            "0x1000",
        ])
        .unwrap();
        assert!(matches!(
            args.command,
            Commands::FlashImage {
                base_offset: 0x1000,
                ..
            }
        ));
    }

    #[test]
    fn test_generate_key_blob_cli_arguments() {
        let mut dek_file = NamedTempFile::new().unwrap();
        dek_file.write_all(&[1, 2, 3]).unwrap();
        let args = parse_args_from([
            "rblhost",
            "--port",
            "COM1",
            "generate-key-blob",
            dek_file.path().to_str().unwrap(),
            "blob.bin",
            "ZMK",
        ])
        .unwrap();
        match args.command {
            Commands::GenerateKeyBlob {
                dek_file,
                blob_file,
                key_sel,
            } => {
                assert_eq!(&*dek_file, &[1, 2, 3]);
                assert_eq!(blob_file, "blob.bin");
                assert_eq!(key_sel, 2);
            }
            other => panic!("expected GenerateKeyBlob, got {other:?}"),
        }
    }

    #[test]
    fn test_read_memory_hexdump_short_option_matches_blhost() {
        let args = parse_args_from(["rblhost", "--port", "COM1", "read-memory", "0", "4", "-h"]).unwrap();
        assert!(matches!(args.command, Commands::ReadMemory { use_hexdump: true, .. }));
    }

    #[test]
    fn test_fuse_read_hexdump_short_option_matches_blhost() {
        let args = parse_args_from(["rblhost", "--port", "COM1", "fuse-read", "0", "4", "-h"]).unwrap();
        assert!(matches!(args.command, Commands::FuseRead { use_hexdump: true, .. }));
    }

    #[test]
    fn test_read_key_store_hexdump_short_option_matches_blhost() {
        let args = parse_args_from([
            "rblhost",
            "--port",
            "COM1",
            "key-provisioning",
            "read_key_store",
            "key-store.bin",
            "-h",
        ])
        .unwrap();
        assert!(matches!(
            args.command,
            Commands::KeyProvisioning(KeyProvOperation::ReadKeyStore { use_hexdump: true, .. })
        ));

        let args = parse_args_from([
            "rblhost",
            "--port",
            "COM1",
            "key-provisioning",
            "read_key_store",
            "key-store.bin",
            "--use-hexdump",
        ])
        .unwrap();
        assert!(matches!(
            args.command,
            Commands::KeyProvisioning(KeyProvOperation::ReadKeyStore { use_hexdump: true, .. })
        ));

        let args = parse_args_from([
            "rblhost",
            "--port",
            "COM1",
            "key-provisioning",
            "read_key_store",
            "key-store.bin",
            "--use_hexdump",
        ]);
        assert!(matches!(
            args.unwrap().command,
            Commands::KeyProvisioning(KeyProvOperation::ReadKeyStore { use_hexdump: true, .. })
        ));
    }

    #[test]
    fn test_hexdump_commands_keep_long_help() {
        let error = parse_args_from(["rblhost", "--port", "COM1", "read-memory", "--help"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_read_key_store_keeps_long_help() {
        let error = parse_args_from([
            "rblhost",
            "--port",
            "COM1",
            "key-provisioning",
            "read_key_store",
            "--help",
        ])
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_memory_bytes_are_formatted_in_lowercase() {
        assert_eq!(format_memory_bytes(&[0x00, 0xA1, 0xFF], false), "00 a1 ff ");
        let bytes = [0u8; 17];
        assert_eq!(
            format_memory_bytes(&bytes, false),
            format!("{} \n00 ", "00 ".repeat(16).trim_end())
        );
    }
    /// `flash-program-once` positional argument order must match blhost's
    /// `INDEX BYTE_COUNT DATA [ENDIANNESS]` (i.e. `INDEX COUNT DATA [--verify]`), not accidentally
    /// swap `count` and `data`.
    #[test]
    fn test_flash_program_once_arg_order_matches_blhost() {
        let args = parse_args_from(["rblhost", "--port", "COM1", "flash-program-once", "0x5", "8", "0x10"]).unwrap();
        match args.command {
            Commands::FlashProgramOnce {
                index,
                count,
                data,
                verify,
            } => {
                assert_eq!(index, 0x5);
                assert_eq!(count, 8);
                assert_eq!(data, 0x10);
                assert!(!verify);
            }
            other => panic!("expected FlashProgramOnce, got {other:?}"),
        }
    }

    #[test]
    fn test_flash_program_once_count_required() {
        // blhost's `byte_count` positional argument is required (no default), so rblhost should
        // behave the same way instead of silently defaulting to 4.
        let result = parse_args_from(["rblhost", "--port", "COM1", "flash-program-once", "0x5", "0x10"]);
        assert!(result.is_err(), "count argument should be required, matching blhost");
    }

    #[test]
    fn test_flash_program_once_count_must_be_4_or_8() {
        // blhost restricts byte_count to `click.Choice(["4", "8"])`; rblhost must reject any
        // other value instead of silently accepting it (e.g. reinterpreting a stale 3-arg
        // `INDEX DATA COUNT` invocation as `INDEX COUNT DATA`).
        for count in ["1", "0x12", "16"] {
            let result = parse_args_from(["rblhost", "--port", "COM1", "flash-program-once", "0x5", count, "0x10"]);
            assert!(
                result.is_err(),
                "count {count} should be rejected, only 4 or 8 are valid"
            );
        }
    }

    #[test]
    fn test_flash_program_once_accepts_64bit_data_for_count_8() {
        // count == 8 must allow a full 64-bit value, not just a 32-bit one.
        let args = parse_args_from([
            "rblhost",
            "--port",
            "COM1",
            "flash-program-once",
            "0x5",
            "8",
            "0x1122334455667788",
        ])
        .unwrap();
        match args.command {
            Commands::FlashProgramOnce { count, data, .. } => {
                assert_eq!(count, 8);
                assert_eq!(data, 0x1122_3344_5566_7788);
            }
            other => panic!("expected FlashProgramOnce, got {other:?}"),
        }
    }

    #[test]
    fn test_flash_read_once_count_must_be_4_or_8() {
        for count in ["1", "0x12", "16"] {
            let result = parse_args_from(["rblhost", "--port", "COM1", "flash-read-once", "0x5", count]);
            assert!(
                result.is_err(),
                "count {count} should be rejected, only 4 or 8 are valid"
            );
        }
    }

    /// `efuse-program-once` must accept blhost's `ADDRESS DATA [nolock/lock]` form, parsing
    /// `DATA` as hex even without a `0x` prefix so that blhost scripts burn the same value.
    #[test]
    fn test_efuse_program_once_matches_blhost() {
        let args = Args::parse_from(["rblhost", "--port", "COM1", "efuse-program-once", "0x60", "12"]);
        match args.command {
            Commands::EfuseProgramOnce {
                address,
                data,
                lock,
                verify,
            } => {
                assert_eq!(address, 0x60);
                assert_eq!(data, 0x12);
                assert_eq!(lock, EfuseLock::Nolock);
                assert!(!verify);
            }
            other => panic!("expected EfuseProgramOnce, got {other:?}"),
        }

        let args = Args::parse_from([
            "rblhost",
            "--port",
            "COM1",
            "efuse-program-once",
            "96",
            "0xDEADBEEF",
            "lock",
            "--verify",
        ]);
        match args.command {
            Commands::EfuseProgramOnce {
                address,
                data,
                lock,
                verify,
            } => {
                assert_eq!(address, 96);
                assert_eq!(data, 0xDEAD_BEEF);
                assert_eq!(lock, EfuseLock::Lock);
                assert!(verify);
            }
            other => panic!("expected EfuseProgramOnce, got {other:?}"),
        }
    }

    #[test]
    fn test_efuse_program_once_rejects_invalid_input() {
        for extra in [
            &["0x60", "1FFFFFFFF"][..],
            &["0x60", "zz"],
            &["0x60", "12", "maybe"],
            &["0x60"],
        ] {
            let mut argv = vec!["rblhost", "--port", "COM1", "efuse-program-once"];
            argv.extend_from_slice(extra);
            assert!(Args::try_parse_from(argv).is_err(), "{extra:?} should be rejected");
        }
    }

    #[test]
    fn test_efuse_read_once() {
        let args = Args::parse_from(["rblhost", "--port", "COM1", "efuse-read-once", "0x60"]);
        match args.command {
            Commands::EfuseReadOnce { address } => assert_eq!(address, 0x60),
            other => panic!("expected EfuseReadOnce, got {other:?}"),
        }
        assert!(Args::try_parse_from(["rblhost", "--port", "COM1", "efuse-read-once"]).is_err());
    }
}
