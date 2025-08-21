// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
//! Bootloader status code definitions.
//!
//! This module contains the enumeration of all possible status codes that can be returned
//! by the bootloader during command execution. Status codes indicate success, failure,
//! or specific error conditions across various subsystems including flash drivers,
//! communication interfaces, memory operations, and security functions.
#![allow(
    clippy::doc_markdown,
    reason = "Some comments do not include any meaningful identifiers that would need to be enclosed in backticks."
)]

#[cfg(feature = "python")]
use pyo3::pyclass;
#[cfg(feature = "python")]
use pyo3_stub_gen::derive::gen_stub_pyclass_enum;
/// Bootloader status codes enumeration.
///
/// Represents all possible status codes that can be returned by the bootloader.
/// Status codes are organized by subsystem and indicate the result of command execution.
#[repr(u32)]
#[derive(derive_more::Display, derive_more::TryFrom, Debug, Clone, Copy, strum::EnumIs, PartialEq, Eq)]
#[try_from(repr)]
#[cfg_attr(feature = "python", gen_stub_pyclass_enum)]
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
pub enum StatusCode {
    /// Command executed successfully
    Success = 0,
    /// General failure
    Fail = 1,
    /// Operation not allowed on read-only target
    ReadOnly = 2,
    /// Parameter value is out of range
    OutOfRange = 3,
    /// Invalid argument provided
    InvalidArgument = 4,
    /// Operation timed out
    Timeout = 5,
    /// No data transfer currently in progress
    NoTransferInProgress = 6,

    // Flash driver errors
    /// Flash driver: Size parameter is invalid
    #[display("FLASH Driver: Size Error")]
    FlashSizeError = 100,
    /// Flash driver: Address alignment is incorrect
    #[display("FLASH Driver: Alignment Error")]
    FlashAlignmentError = 101,
    /// Flash driver: Address is invalid or out of range
    #[display("FLASH Driver: Address Error")]
    FlashAddressError = 102,
    /// Flash driver: Access to flash memory failed
    #[display("FLASH Driver: Access Error")]
    FlashAccessError = 103,
    /// Flash driver: Write protection violation
    #[display("FLASH Driver: Protection Violation")]
    FlashProtectionViolation = 104,
    /// Flash driver: Flash command execution failed
    #[display("FLASH Driver: Command Failure")]
    FlashCommandFailure = 105,
    /// Flash driver: Unknown property requested
    #[display("FLASH Driver: Unknown Property")]
    FlashUnknownProperty = 106,
    /// Flash driver: Erase key does not match programmed key
    #[display("FLASH Driver: Provided Key Does Not Match Programmed Flash Memory Key")]
    FlashEraseKeyError = 107,
    /// Flash driver: Region is execute-only
    #[display("FLASH Driver: Region Execute Only")]
    FlashRegionExecuteOnly = 108,
    /// Flash driver: Execute-in-RAM function not ready
    #[display("FLASH Driver: Execute In RAM Function Not Ready")]
    FlashExecInRamNotReady = 109,
    /// Flash driver: Command not supported
    #[display("FLASH Driver: Command Not Supported")]
    FlashCommandNotSupported = 111,
    /// Flash driver: Property is read-only
    #[display("FLASH Driver: Flash Memory Property Is Read-Only")]
    FlashReadOnlyProperty = 112,
    /// Flash driver: Property value is out of range
    #[display("FLASH Driver: Flash Memory Property Value Out Of Range")]
    FlashInvalidPropertyValue = 113,
    /// Flash driver: Prefetch speculation option is invalid
    #[display("FLASH Driver: Flash Memory Prefetch Speculation Option Is Invalid")]
    FlashInvalidSpeculationOption = 114,
    /// Flash driver: ECC error detected
    #[display("FLASH Driver: ECC Error")]
    FlashEccError = 116,
    /// Flash driver: Memory contents do not match after verification
    #[display("FLASH Driver: Destination And Source Memory Contents Do Not Match")]
    FlashCompareError = 117,
    /// Flash driver: Regulation lost during read operation
    #[display("FLASH Driver: Loss Of Regulation During Read")]
    FlashRegulationLoss = 118,
    /// Flash driver: Wait state cycles setting is invalid
    #[display("FLASH Driver: Wait State Cycle Set To Read/Write Mode Is Invalid")]
    FlashInvalidWaitStateCycles = 119,
    /// Flash driver: CFPA page is out of date
    #[display("FLASH Driver: Out Of Date CFPA Page")]
    FlashOutOfDateCfpaPage = 132,
    /// Flash driver: IFR page data is blank
    #[display("FLASH Driver: Blank IFR Page Data")]
    FlashBlankIfrPageData = 133,
    /// Flash driver: Encrypted regions must be erased together
    #[display("FLASH Driver: Encrypted Regions Erase Not Done At Once")]
    FlashEncryptedRegionsEraseNotDoneAtOnce = 134,
    /// Flash driver: Program verification is not allowed
    #[display("FLASH Driver: Program Verification Not Allowed")]
    FlashProgramVerificationNotAllowed = 135,
    /// Flash driver: Hash check failed
    #[display("FLASH Driver: Hash Check Error")]
    FlashHashCheckError = 136,
    /// Flash driver: PFR region is sealed
    #[display("FLASH Driver: Sealed PFR Region")]
    FlashSealedPfrRegion = 137,
    /// Flash driver: PFR region write is broken
    #[display("FLASH Driver: PFR Region Write Broken")]
    FlashPfrRegionWriteBroken = 138,
    /// Flash driver: NMPA update is not allowed
    #[display("FLASH Driver: NMPA Update Not Allowed")]
    FlashNmpaUpdateNotAllowed = 139,
    /// Flash driver: CMPA configuration direct erase is not allowed
    #[display("FLASH Driver: CMPA Cfg Direct Erase Not Allowed")]
    FlashCmpaCfgDirectEraseNotAllowed = 140,
    /// Flash driver: PFR bank is locked
    #[display("FLASH Driver: PFR Bank Is Locked")]
    FlashPfrBankIsLocked = 141,
    /// Flash driver: CFPA scratch page is invalid
    #[display("FLASH Driver: CFPA Scratch Page Invalid")]
    FlashCfpaScratchPageInvalid = 148,
    /// Flash driver: CFPA version rollback is disallowed
    #[display("FLASH Driver: CFPA Version Rollback Disallowed")]
    FlashCfpaVersionRollbackDisallowed = 149,
    /// Flash driver: Reading from hiding area is disallowed
    #[display("FLASH Driver: Flash Memory Hiding Read Not Allowed")]
    FlashReadHidingAreaDisallowed = 150,
    /// Flash driver: Modifying protected area is disallowed
    #[display("FLASH Driver: Flash Firewall Page Locked Erase And Program Are Not Allowed")]
    FlashModifyProtectedAreaDisallowed = 151,
    /// Flash driver: Flash command operation is in progress
    #[display("FLASH Driver: Flash Memory State Busy Flash Memory Command Is In Progress")]
    FlashCommandOperationInProgress = 152,

    // I2C driver errors
    /// I2C driver: Slave transmit underrun
    #[display("I2C Driver: Slave Tx Underrun")]
    I2cSlaveTxUnderrun = 200,
    /// I2C driver: Slave receive overrun
    #[display("I2C Driver: Slave Rx Overrun")]
    I2cSlaveRxOverrun = 201,
    /// I2C driver: Arbitration lost on bus
    #[display("I2C Driver: Arbitration Lost")]
    I2cArbitrationLost = 202,

    // SPI errors
    /// SPI driver: Slave transmit underrun
    #[display("SPI Driver: Slave Tx Underrun")]
    SpiSlaveTxUnderrun = 300,
    /// SPI driver: Slave receive overrun
    #[display("SPI Driver: Slave Rx Overrun")]
    SpiSlaveRxOverrun = 301,

    // QuadSPI driver errors
    /// QuadSPI driver: Flash size error
    #[display("QSPI Driver: Flash Size Error")]
    QspiFlashSizeError = 400,
    /// QuadSPI driver: Flash alignment error
    #[display("QSPI Driver: Flash Alignment Error")]
    QspiFlashAlignmentError = 401,
    /// QuadSPI driver: Flash address error
    #[display("QSPI Driver: Flash Address Error")]
    QspiFlashAddressError = 402,
    /// QuadSPI driver: Flash command failure
    #[display("QSPI Driver: Flash Command Failure")]
    QspiFlashCommandFailure = 403,
    /// QuadSPI driver: Unknown property
    #[display("QSPI Driver: Flash Unknown Property")]
    QspiFlashUnknownProperty = 404,
    /// QuadSPI driver: Interface not configured
    #[display("QSPI Driver: Not Configured")]
    QspiNotConfigured = 405,
    /// QuadSPI driver: Command not supported
    #[display("QSPI Driver: Command Not Supported")]
    QspiCommandNotSupported = 406,
    /// QuadSPI driver: Command timed out
    #[display("QSPI Driver: Command Timeout")]
    QspiCommandTimeout = 407,
    /// QuadSPI driver: Write operation failed
    #[display("QSPI Driver: Write Failure")]
    QspiWriteFailure = 408,

    // OTFAD driver errors
    /// OTFAD driver: Security violation detected
    #[display("OTFAD Driver: Security Violation")]
    OtfadSecurityViolation = 500,
    /// OTFAD driver: Logically disabled
    #[display("OTFAD Driver: Logically Disabled")]
    OtfadLogicallyDisabled = 501,
    /// OTFAD driver: Invalid key provided
    #[display("OTFAD Driver: Invalid Key")]
    OtfadInvalidKey = 502,
    /// OTFAD driver: Invalid key blob
    #[display("OTFAD Driver: Invalid Key Blob")]
    OtfadInvalidKeyBlob = 503,

    // Sending errors
    /// Send operation condition failed
    #[display("Send Operation Condition failed")]
    SendingOperationConditionError = 1812,

    // FlexSPI statuses
    /// FlexSPI: Sequence execution timeout (RT5xx)
    #[display("FLEXSPI: Sequence Execution Timeout")]
    FlexspiSequenceExecutionTimeoutRt5xx = 6000,
    /// FlexSPI: Invalid sequence (RT5xx)
    #[display("FLEXSPI: Invalid Sequence")]
    FlexspiInvalidSequenceRt5xx = 6001,
    /// FlexSPI: Device timeout (RT5xx)
    #[display("FLEXSPI: Device Timeout")]
    FlexspiDeviceTimeoutRt5xx = 6002,
    /// FlexSPI: Sequence execution timeout
    #[display("FLEXSPI: Sequence Execution Timeout")]
    FlexspiSequenceExecutionTimeout = 7000,
    /// FlexSPI: Invalid sequence
    #[display("FLEXSPI: Invalid Sequence")]
    FlexspiInvalidSequence = 7001,
    /// FlexSPI: Device timeout
    #[display("FLEXSPI: Device Timeout")]
    FlexspiDeviceTimeout = 7002,

    // Bootloader errors
    /// Bootloader: Unknown command received
    #[display("Unknown Command")]
    UnknownCommand = 10000,
    /// Bootloader: Security violation detected
    #[display("Security Violation")]
    SecurityViolation = 10001,
    /// Bootloader: Data phase aborted
    #[display("Abort Data Phase")]
    AbortDataPhase = 10002,
    /// Bootloader: Ping command failed
    #[display("Ping Error")]
    PingError = 10003,
    /// Bootloader: No response packet received from target
    #[display("No response packet from target device")]
    NoResponse = 10004,
    /// Bootloader: No response expected for this command
    #[display("No Response Expected")]
    NoResponseExpected = 10005,
    /// Bootloader: Command is not supported
    #[display("Unsupported Command")]
    UnsupportedCommand = 10006,

    // SB loader errors
    /// ROM loader: Section overrun detected
    #[display("ROM Loader: Section Overrun")]
    RomldrSectionOverrun = 10100,
    /// ROM loader: Signature verification failed
    #[display("ROM Loader: Signature Error")]
    RomldrSignature = 10101,
    /// ROM loader: Section length is invalid
    #[display("ROM Loader: Section Length Error")]
    RomldrSectionLength = 10102,
    /// ROM loader: Only unencrypted content allowed
    #[display("ROM Loader: Unencrypted Only")]
    RomldrUnencryptedOnly = 10103,
    /// ROM loader: End of file reached
    #[display("ROM Loader: EOF Reached")]
    RomldrEofReached = 10104,
    /// ROM loader: Checksum verification failed
    #[display("ROM Loader: Checksum Error")]
    RomldrChecksum = 10105,
    /// ROM loader: CRC32 verification failed
    #[display("ROM Loader: CRC32 Error")]
    RomldrCrc32Error = 10106,
    /// ROM loader: Unknown command received
    #[display("ROM Loader: Unknown Command")]
    RomldrUnknownCommand = 10107,
    /// ROM loader: ID not found
    #[display("ROM Loader: ID Not Found")]
    RomldrIdNotFound = 10108,
    /// ROM loader: Data underrun detected
    #[display("ROM Loader: Data Underrun")]
    RomldrDataUnderrun = 10109,
    /// ROM loader: Jump instruction returned
    #[display("ROM Loader: Jump Returned")]
    RomldrJumpReturned = 10110,
    /// ROM loader: Function call failed
    #[display("ROM Loader: Call Failed")]
    RomldrCallFailed = 10111,
    /// ROM loader: Key not found
    #[display("ROM Loader: Key Not Found")]
    RomldrKeyNotFound = 10112,
    /// ROM loader: Secure mode only
    #[display("ROM Loader: Secure Only")]
    RomldrSecureOnly = 10113,
    /// ROM loader: Reset instruction returned
    #[display("ROM Loader: Reset Returned")]
    RomldrResetReturned = 10114,
    /// ROM loader: Rollback is blocked
    #[display("ROM Loader: Rollback Blocked")]
    RomldrRollbackBlocked = 10115,
    /// ROM loader: Invalid section MAC count
    #[display("ROM Loader: Invalid Section Mac Count")]
    RomldrInvalidSectionMacCount = 10116,
    /// ROM loader: Unexpected command received
    #[display("ROM Loader: Unexpected Command")]
    RomldrUnexpectedCommand = 10117,
    /// ROM loader: Bad SBKEK detected
    #[display("ROM Loader: Bad SBKEK Detected")]
    RomldrBadSbkek = 10118,
    /// ROM loader: Jump command is pending
    #[display("ROM Loader: Pending Jump Command")]
    RomldrPendingJumpCommand = 10119,

    // Memory interface errors
    /// Memory interface: Address range is invalid
    #[display("Memory Range Invalid")]
    MemoryRangeInvalid = 10200,
    /// Memory interface: Read operation failed
    #[display("Memory Read Failed")]
    MemoryReadFailed = 10201,
    /// Memory interface: Write operation failed
    #[display("Memory Write Failed")]
    MemoryWriteFailed = 10202,
    /// Memory interface: Cumulative write detected
    #[display("Memory Cumulative Write")]
    MemoryCumulativeWrite = 10203,
    /// Memory interface: Application overlaps with execute-only region
    #[display("Memory App Overlap with exec region")]
    MemoryAppOverlapWithExecuteOnlyRegion = 10204,
    /// Memory interface: Memory not configured
    #[display("Memory Not Configured")]
    MemoryNotConfigured = 10205,
    /// Memory interface: Address alignment error
    #[display("Memory Alignment Error")]
    MemoryAlignmentError = 10206,
    /// Memory interface: Verification failed
    #[display("Memory Verify Failed")]
    MemoryVerifyFailed = 10207,
    /// Memory interface: Memory is write protected
    #[display("Memory Write Protected")]
    MemoryWriteProtected = 10208,
    /// Memory interface: Address is invalid
    #[display("Memory Address Error")]
    MemoryAddressError = 10209,
    /// Memory interface: Blank check failed
    #[display("Memory Black Check Failed")]
    MemoryBlankCheckFailed = 10210,
    /// Memory interface: Blank page read is disallowed
    #[display("Memory Blank Page Read Disallowed")]
    MemoryBlankPageReadDisallowed = 10211,
    /// Memory interface: Protected page read is disallowed
    #[display("Memory Protected Page Read Disallowed")]
    MemoryProtectedPageReadDisallowed = 10212,
    /// Memory interface: PFR spec region write is broken
    #[display("Memory PFR Spec Region Write Broken")]
    MemoryPfrSpecRegionWriteBroken = 10213,
    /// Memory interface: Command not supported
    #[display("Memory Unsupported Command")]
    MemoryUnsupportedCommand = 10214,

    // Property store errors
    /// Property store: Unknown property requested
    #[display("Unknown Property")]
    UnknownProperty = 10300,
    /// Property store: Property is read-only
    #[display("Read Only Property")]
    ReadOnlyProperty = 10301,
    /// Property store: Property value is invalid
    #[display("Invalid Property Value")]
    InvalidPropertyValue = 10302,

    // CRC errors
    /// Application CRC check passed
    #[display("Application CRC Check: Passed")]
    AppCrcCheckPassed = 10400,
    /// Application CRC check failed
    #[display("Application: CRC Check: Failed")]
    AppCrcCheckFailed = 10401,
    /// Application CRC check is inactive
    #[display("Application CRC Check: Inactive")]
    AppCrcCheckInactive = 10402,
    /// Application CRC check is invalid
    #[display("Application CRC Check: Invalid")]
    AppCrcCheckInvalid = 10403,
    /// Application CRC check is out of range
    #[display("Application CRC Check: Out Of Range")]
    AppCrcCheckOutOfRange = 10404,

    // Packetizer errors
    /// Packetizer: No ping response received
    #[display("Packetizer Error: No Ping Response")]
    PacketizerNoPingResponse = 10500,
    /// Packetizer: Invalid packet type
    #[display("Packetizer Error: No response received for ping command")]
    PacketizerInvalidPacketType = 10501,
    /// Packetizer: Invalid CRC
    #[display("Packetizer Error: Invalid packet type")]
    PacketizerInvalidCrc = 10502,
    /// Packetizer: No command response received
    #[display("Packetizer Error: No response received for command")]
    PacketizerNoCommandResponse = 10503,

    // Reliable Update statuses
    /// Reliable update: Operation successful
    #[display("Reliable Update: Success")]
    ReliableUpdateSuccess = 10600,
    /// Reliable update: Operation failed
    #[display("Reliable Update: Fail")]
    ReliableUpdateFail = 10601,
    /// Reliable update: Feature is inactive
    #[display("Reliable Update: Inactive")]
    ReliableUpdateInactive = 10602,
    /// Reliable update: Backup application is invalid
    #[display("Reliable Update: Backup Application Invalid")]
    ReliableUpdateBackupapplicationinvalid = 10603,
    /// Reliable update: Still in main application
    #[display("Reliable Update: Still In Main Application")]
    ReliableUpdateStillinmainapplication = 10604,
    /// Reliable update: Swap system is not ready
    #[display("Reliable Update: Swap System Not Ready")]
    ReliableUpdateSwapsystemnotready = 10605,
    /// Reliable update: Backup bootloader is not ready
    #[display("Reliable Update: Backup Bootloader Not Ready")]
    ReliableUpdateBackupbootloadernotready = 10606,
    /// Reliable update: Swap indicator address is invalid
    #[display("Reliable Update: Swap Indicator Address Invalid")]
    ReliableUpdateSwapindicatoraddressinvalid = 10607,
    /// Reliable update: Swap system is not available
    #[display("Reliable Update: Swap System Not Available")]
    ReliableUpdateSwapsystemnotavailable = 10608,
    /// Reliable update: Swap test mode
    #[display("Reliable Update: Swap Test")]
    ReliableUpdateSwaptest = 10609,

    // Serial NOR/EEPROM statuses
    /// Serial NOR/EEPROM: Address is invalid
    #[display("SerialNorEeprom: Address Invalid")]
    SerialNorEepromAddressInvalid = 10700,
    /// Serial NOR/EEPROM: Transfer error occurred
    #[display("SerialNorEeprom: Transfer Error")]
    SerialNorEepromTransferError = 10701,
    /// Serial NOR/EEPROM: Type is invalid
    #[display("SerialNorEeprom: Type Invalid")]
    SerialNorEepromTypeInvalid = 10702,
    /// Serial NOR/EEPROM: Size is invalid
    #[display("SerialNorEeprom: Size Invalid")]
    SerialNorEepromSizeInvalid = 10703,
    /// Serial NOR/EEPROM: Command is invalid
    #[display("SerialNorEeprom: Command Invalid")]
    SerialNorEepromCommandInvalid = 10704,

    // ROM API statuses
    /// ROM API: Need more data
    #[display("RomApi: Need More Data")]
    RomApiNeedMoreData = 10800,
    /// ROM API: Buffer size is not enough
    #[display("RomApi: Buffer Size Not Enough")]
    RomApiBufferSizeNotEnough = 10801,
    /// ROM API: Invalid buffer provided
    #[display("RomApi: Invalid Buffer")]
    RomApiInvalidBuffer = 10802,

    // FlexSPI NAND statuses
    /// FlexSPI NAND: Read page failed
    #[display("FlexSPINAND: Read Page Fail")]
    FlexspinandReadPageFail = 20000,
    /// FlexSPI NAND: Read cache failed
    #[display("FlexSPINAND: Read Cache Fail")]
    FlexspinandReadCacheFail = 20001,
    /// FlexSPI NAND: ECC check failed
    #[display("FlexSPINAND: Ecc Check Fail")]
    FlexspinandEccCheckFail = 20002,
    /// FlexSPI NAND: Page load failed
    #[display("FlexSPINAND: Page Load Fail")]
    FlexspinandPageLoadFail = 20003,
    /// FlexSPI NAND: Page execute failed
    #[display("FlexSPINAND: Page Execute Fail")]
    FlexspinandPageExecuteFail = 20004,
    /// FlexSPI NAND: Erase block failed
    #[display("FlexSPINAND: Erase Block Fail")]
    FlexspinandEraseBlockFail = 20005,
    /// FlexSPI NAND: Wait timeout occurred
    #[display("FlexSPINAND: Wait Timeout")]
    FlexspinandWaitTimeout = 20006,
    /// FlexSPI NAND: Page size exceeds maximum supported
    #[display("SPI NAND: PageSize over the max supported size")]
    FlexSpinandNotSupported = 20007,
    /// FlexSPI NAND: FCB update failed
    #[display("SPI NAND: Failed to update Flash config block to SPI NAND")]
    FlexSpinandFcbUpdateFail = 20008,
    /// FlexSPI NAND: DBBT update failed
    #[display("SPI NAND: Failed to update discovered bad block table to SPI NAND")]
    FlexSpinandDbbtUpdateFail = 20009,
    /// FlexSPI NAND: Write alignment error
    #[display("FlexSPINAND: Write Alignment Error")]
    FlexspinandWritealignmenterror = 20010,
    /// FlexSPI NAND: Device not found
    #[display("FlexSPINAND: Not Found")]
    FlexspinandNotFound = 20011,

    // FlexSPI NOR statuses
    /// FlexSPI NOR: Program operation failed
    #[display("FLEXSPINOR: Program Fail")]
    FlexspinorProgramFail = 20100,
    /// FlexSPI NOR: Erase sector failed
    #[display("FLEXSPINOR: Erase Sector Fail")]
    FlexspinorEraseSectorFail = 20101,
    /// FlexSPI NOR: Erase all failed
    #[display("FLEXSPINOR: Erase All Fail")]
    FlexspinorEraseAllFail = 20102,
    /// FlexSPI NOR: Wait timeout occurred
    #[display("FLEXSPINOR:Wait Timeout")]
    FlexspinorWaitTimeout = 20103,
    /// FlexSPI NOR: Page size exceeds maximum supported
    #[display("FlexSPINOR: PageSize over the max supported size")]
    FlexspinorNotSupported = 20104,
    /// FlexSPI NOR: Write alignment error
    #[display("FlexSPINOR:Write Alignment Error")]
    FlexspinorWriteAlignmentError = 20105,
    /// FlexSPI NOR: Command failure
    #[display("FlexSPINOR: Command Failure")]
    FlexspinorCommandFailure = 20106,
    /// FlexSPI NOR: SFDP not found
    #[display("FlexSPINOR: SFDP Not Found")]
    FlexspinorSfdpNotFound = 20107,
    /// FlexSPI NOR: Unsupported SFDP version
    #[display("FLEXSPINOR: Unsupported SFDP Version")]
    FlexspinorUnsupportedSfdpVersion = 20108,
    /// FlexSPI NOR: Flash not found
    #[display("FLEXSPINOR Flash Not Found")]
    FlexspinorFlashNotFound = 20109,
    /// FlexSPI NOR: DTR read dummy probe failed
    #[display("FLEXSPINOR: DTR Read Dummy Probe Failed")]
    FlexspinorDtrReadDummyProbeFailed = 20110,

    // OCOTP statuses
    /// OCOTP: Read failure
    #[display("OCOTP: Read Failure")]
    OcotpReadFailure = 20200,
    /// OCOTP: Program failure
    #[display("OCOTP: Program Failure")]
    OcotpProgramFailure = 20201,
    /// OCOTP: Reload failure
    #[display("OCOTP: Reload Failure")]
    OcotpReloadFailure = 20202,
    /// OCOTP: Wait timeout occurred
    #[display("OCOTP: Wait Timeout")]
    OcotpWaitTimeout = 20203,

    // SEMC NOR statuses
    /// SEMC NOR: Device timeout
    #[display("SemcNOR: Device Timeout")]
    SemcnorDeviceTimeout = 21100,
    /// SEMC NOR: Invalid memory address
    #[display("SemcNOR: Invalid Memory Address")]
    SemcnorInvalidMemoryAddress = 21101,
    /// SEMC NOR: Unmatched command set
    #[display("SemcNOR: unmatched Command Set")]
    SemcnorUnmatchedCommandSet = 21102,
    /// SEMC NOR: Address alignment error
    #[display("SemcNOR: Address Alignment Error")]
    SemcnorAddressAlignmentError = 21103,
    /// SEMC NOR: Invalid CFI signature
    #[display("SemcNOR: Invalid Cfi Signature")]
    SemcnorInvalidCfiSignature = 21104,
    /// SEMC NOR: Command error - no operation to suspend
    #[display("SemcNOR: Command Error No Op To Suspend")]
    SemcnorCommandErrorNoOpToSuspend = 21105,
    /// SEMC NOR: Command error - no information available
    #[display("SemcNOR: Command Error No Info Available")]
    SemcnorCommandErrorNoInfoAvailable = 21106,
    /// SEMC NOR: Block erase command failure
    #[display("SemcNOR: Block Erase Command Failure")]
    SemcnorBlockEraseCommandFailure = 21107,
    /// SEMC NOR: Buffer program command failure
    #[display("SemcNOR: Buffer Program Command Failure")]
    SemcnorBufferProgramCommandFailure = 21108,
    /// SEMC NOR: Program verify failure
    #[display("SemcNOR: Program Verify Failure")]
    SemcnorProgramVerifyFailure = 21109,
    /// SEMC NOR: Erase verify failure
    #[display("SemcNOR: Erase Verify Failure")]
    SemcnorEraseVerifyFailure = 21110,
    /// SEMC NOR: Invalid configuration tag
    #[display("SemcNOR: Invalid Cfg Tag")]
    SemcnorInvalidCfgTag = 21116,

    // SEMC NAND statuses
    /// SEMC NAND: Device timeout
    #[display("SemcNAND: Device Timeout")]
    SemcnandDeviceTimeout = 21200,
    /// SEMC NAND: Invalid memory address
    #[display("SemcNAND: Invalid Memory Address")]
    SemcnandInvalidMemoryAddress = 21201,
    /// SEMC NAND: Not equal to one page size
    #[display("SemcNAND: Not Equal To One Page Size")]
    SemcnandNotEqualToOnePageSize = 21202,
    /// SEMC NAND: More than one page size
    #[display("SemcNAND: More Than One Page Size")]
    SemcnandMoreThanOnePageSize = 21203,
    /// SEMC NAND: ECC check failed
    #[display("SemcNAND: Ecc Check Fail")]
    SemcnandEccCheckFail = 21204,
    /// SEMC NAND: Invalid ONFI parameter
    #[display("SemcNAND: Invalid Onfi Parameter")]
    SemcnandInvalidOnfiParameter = 21205,
    /// SEMC NAND: Cannot enable device ECC
    #[display("SemcNAND: Cannot Enable Device Ecc")]
    SemcnandCannotEnableDeviceEcc = 21206,
    /// SEMC NAND: Switch timing mode failure
    #[display("SemcNAND: Switch Timing Mode Failure")]
    SemcnandSwitchTimingModeFailure = 21207,
    /// SEMC NAND: Program verify failure
    #[display("SemcNAND: Program Verify Failure")]
    SemcnandProgramVerifyFailure = 21208,
    /// SEMC NAND: Erase verify failure
    #[display("SemcNAND: Erase Verify Failure")]
    SemcnandEraseVerifyFailure = 21209,
    /// SEMC NAND: Invalid readback buffer
    #[display("SemcNAND: Invalid Readback Buffer")]
    SemcnandInvalidReadbackBuffer = 21210,
    /// SEMC NAND: Invalid configuration tag
    #[display("SemcNAND: Invalid Cfg Tag")]
    SemcnandInvalidCfgTag = 21216,
    /// SEMC NAND: Failed to update FCB
    #[display("SemcNAND: Fail To Update Fcb")]
    SemcnandFailToUpdateFcb = 21217,
    /// SEMC NAND: Failed to update DBBT
    #[display("SemcNAND: Fail To Update Dbbt")]
    SemcnandFailToUpdateDbbt = 21218,
    /// SEMC NAND: Disallow overwrite BCB
    #[display("SemcNAND: Disallow Overwrite Bcb")]
    SemcnandDisallowOverwriteBcb = 21219,
    /// SEMC NAND: Only support ONFI device
    #[display("SemcNAND: Only Support Onfi Device")]
    SemcnandOnlySupportOnfiDevice = 21220,
    /// SEMC NAND: More than max image copy
    #[display("SemcNAND: More Than Max Image Copy")]
    SemcnandMoreThanMaxImageCopy = 21221,
    /// SEMC NAND: Disordered image copies
    #[display("SemcNAND: Disordered Image Copies")]
    SemcnandDisorderedImageCopies = 21222,

    // SPIFI NOR statuses
    /// SPIFI NOR: Program operation failed
    #[display("SPIFINOR: Program Fail")]
    SpifinorProgramFail = 22000,
    /// SPIFI NOR: Erase sector failed
    #[display("SPIFINOR: Erase Sector Fail")]
    SpifinorEraseSectorfail = 22001,
    /// SPIFI NOR: Erase all failed
    #[display("SPIFINOR: Erase All Fail")]
    SpifinorEraseAllFail = 22002,
    /// SPIFI NOR: Wait timeout occurred
    #[display("SPIFINOR: Wait Timeout")]
    SpifinorWaitTimeout = 22003,
    /// SPIFI NOR: Operation not supported
    #[display("SPIFINOR: Not Supported")]
    SpifinorNotSupported = 22004,
    /// SPIFI NOR: Write alignment error
    #[display("SPIFINOR: Write Alignment Error")]
    SpifinorWriteAlignmentError = 22005,
    /// SPIFI NOR: Command failure
    #[display("SPIFINOR: Command Failure")]
    SpifinorCommandFailure = 22006,
    /// SPIFI NOR: SFDP not found
    #[display("SPIFINOR: SFDP Not Found")]
    SpifinorSfdpNotFound = 22007,

    // EDGELOCK ENCLAVE statuses
    /// EdgeLock Enclave: Invalid response
    #[display("EDGELOCK: Invalid Response")]
    EdgelockInvalidResponse = 30000,
    /// EdgeLock Enclave: Response error
    #[display("EDGELOCK: Response Error")]
    EdgelockResponseError = 30001,
    /// EdgeLock Enclave: Operation aborted
    #[display("EDGELOCK: Abort")]
    EdgelockAbort = 30002,
    /// EdgeLock Enclave: Operation failed
    #[display("EDGELOCK: Operation Failed")]
    EdgelockOperationFailed = 30003,
    /// EdgeLock Enclave: OTP program failure
    #[display("EDGELOCK: OTP Program Failure")]
    EdgelockOtpProgramFailure = 30004,
    /// EdgeLock Enclave: OTP is locked
    #[display("EDGELOCK: OTP Locked")]
    EdgelockOtpLocked = 30005,
    /// EdgeLock Enclave: OTP invalid index
    #[display("EDGELOCK: OTP Invalid IDX")]
    EdgelockOtpInvalidIdx = 30006,
    /// EdgeLock Enclave: Invalid lifecycle state
    #[display("EDGELOCK: Invalid Lifecycle")]
    EdgelockInvalidLifecycle = 30007,

    // OTP statuses
    /// OTP: Invalid OTP address
    #[display("OTD: Invalid OTP address")]
    OtpInvalidAddress = 52801,
    /// OTP: Programming failed
    #[display("OTD: Programming failed")]
    OtpProgramFail = 52802,
    /// OTP: CRC check failed
    #[display("OTP: CRC check failed")]
    OtpCrcFail = 52803,
    /// OTP: Error occurred during operation
    #[display("OTP: Error happened during OTP operation")]
    OtpError = 52804,
    /// OTP: ECC check failed during operation
    #[display("OTP: ECC check failed during OTP operation")]
    OtpEccCrcFail = 52805,
    /// OTP: Field is locked when programming
    #[display("OTP: Field is locked when programming")]
    OtpLocked = 52806,
    /// OTP: Operation timed out
    #[display("OTP: Operation timed out")]
    OtpTimeout = 52807,
    /// OTP: CRC check passed
    #[display("OTP: CRC check passed")]
    OtpCrcCheckPass = 52808,
    /// OTP: Failed to verify OTP write
    #[display("OTP: Failed to verify OTP write")]
    OtpVerifyFail = 52009,

    // Security subsystem statuses
    /// Security subsystem error
    #[display("Security SubSystem Error")]
    SecuritySubsystemError = 1515890085,

    // TrustProvisioning statuses
    /// Trust Provisioning: General error
    #[display("TP: General error")]
    TpGeneralError = 80000,
    /// Trust Provisioning: Cryptographic operation error
    #[display("TP: Error during cryptographic operation")]
    TpCryptoError = 80001,
    /// Trust Provisioning: Null pointer dereference or buffer allocation failed
    #[display("TP: NULL pointer dereference or when buffer could not be allocated")]
    TpNullptrError = 80002,
    /// Trust Provisioning: Already initialized
    #[display("TP: Already initialized")]
    TpAlreadyinitialized = 80003,
    /// Trust Provisioning: Buffer is too small
    #[display("TP: Buffer is too small")]
    TpBuffersmall = 80004,
    /// Trust Provisioning: Address out of range or buffer allocation failed
    #[display("TP: Address out of allowed range or buffer could not be allocated")]
    TpAddressError = 80005,
    /// Trust Provisioning: Container header or size is invalid
    #[display("TP: Container header or size is invalid")]
    TpContainerInvalid = 80006,
    /// Trust Provisioning: Container entry invalid
    #[display("TP: Container entry invalid")]
    TpContainerentryinvalid = 80007,
    /// Trust Provisioning: Container entry not found
    #[display("TP: Container entry not found in container")]
    TpContainerentrynotfound = 80008,
    /// Trust Provisioning: Invalid state operation
    #[display("TP: Attempt to process command in disallowed state")]
    TpInvalidstateoperation = 80009,
    /// Trust Provisioning: ISP command arguments are invalid
    #[display("TP: ISP command arguments are invalid")]
    TpCommandError = 80010,
    /// Trust Provisioning: PUF operation error
    #[display("TP: PUF operation error")]
    TpPufError = 80011,
    /// Trust Provisioning: Flash operation failed
    #[display("TP: Flash erase/program/verify_erase failed")]
    TpFlashError = 80012,
    /// Trust Provisioning: Secret box error
    #[display("TP: SBKEK or USER KEK cannot be stored in secret box")]
    TpSecretboxError = 80013,
    /// Trust Provisioning: PFR operation failed
    #[display("TP: Protected Flash Region operation failed")]
    TpPfrError = 80014,
    /// Trust Provisioning: Container signature verification failed
    #[display("TP: Container signature verification failed")]
    TpVerificationError = 80015,
    /// Trust Provisioning: CFPA page cannot be stored
    #[display("TP: CFPA page cannot be stored")]
    TpCfpaError = 80016,
    /// Trust Provisioning: CMPA page cannot be stored
    #[display("TP: CMPA page cannot be stored or ROTKH or SECU registers are invalid")]
    TpCmpaError = 80017,
    /// Trust Provisioning: Address is out of range
    #[display("TP: Address is out of range")]
    TpAddrOutOfRange = 80018,
    /// Trust Provisioning: Container address error
    #[display("TP: Container address in write context is invalid or there is no memory for entry storage")]
    TpContainerAddrError = 80019,
    /// Trust Provisioning: Container address unaligned
    #[display("TP: Container address in read context is unaligned")]
    TpContainerAddrUnaligned = 80020,
    /// Trust Provisioning: Container buffer too small
    #[display("TP: There is not enough memory to store the container")]
    TpContainerBuffSmall = 80021,
    /// Trust Provisioning: Container has no entry
    #[display("TP: Attempt to sign an empty container")]
    TpContainerNoEntry = 80022,
    /// Trust Provisioning: Certificate address error
    #[display("TP: Destination address of OEM certificate is invalid")]
    TpCertAddrError = 80023,
    /// Trust Provisioning: Certificate address unaligned
    #[display("TP: Destination address of certificate is unaligned")]
    TpCertAddrUnaligned = 80024,
    /// Trust Provisioning: Certificate overlapping
    #[display("TP: OEM certificates are overlapping due to wrong destination addresses")]
    TpCertOverlapping = 80025,
    /// Trust Provisioning: Packet error
    #[display("TP: Error during packet sending/receiving")]
    TpPacketError = 80026,
    /// Trust Provisioning: Packet data error
    #[display("TP: Data in packet handle are invalid")]
    TpPacketDataError = 80027,
    /// Trust Provisioning: Unknown command
    #[display("TP: Unknown command was received")]
    TpUnknownCommand = 80028,
    /// Trust Provisioning: SB3 file error
    #[display("TP: Error during processing SB3 file")]
    TpSb3FileError = 80029,
    /// Trust Provisioning: General critical error
    #[display("TP: Critical error")]
    TpGeneralCriticalError = 80101,
    /// Trust Provisioning: Crypto critical error
    #[display("TP: Error of crypto module which prevents proper functionality")]
    TpCryptoCriticalError = 80102,
    /// Trust Provisioning: PUF critical error
    #[display("TP: Initialization or start of the PUF periphery failed")]
    TpPufCriticalError = 80103,
    /// Trust Provisioning: PFR critical error
    #[display("TP: Initialization of PFR or reading of activation code failed")]
    TpPfrCriticalError = 80104,
    /// Trust Provisioning: Peripheral critical error
    #[display("TP: Peripheral failure")]
    TpPeripheralCriticalError = 80105,
    /// Trust Provisioning: Prince critical error
    #[display("TP: Error during PRINCE encryption/decryption")]
    TpPrinceCriticalError = 80106,
    /// Trust Provisioning: SHA check verification failed
    #[display("TP: SHA check verification failed")]
    TpShaCheckCriticalError = 80107,

    // IAP statuses
    /// IAP: Invalid argument detected during API execution
    #[display("IAP: Invalid Argument Detected During API Execution")]
    IapInvalidArgument = 100001,
    /// IAP: Heap size not large enough during API execution
    #[display("IAP: Heap Size Not Large Enough During API Execution")]
    IapOutOfMemory = 100002,
    /// IAP: Read memory operation disallowed during API execution
    #[display("IAP: Read Memory Operation Disallowed During API Execution")]
    IapReadDisallowed = 100003,
    /// IAP: Flash memory region to be programmed is not empty
    #[display("IAP: Flash Memory Region To Be Programmed Is Not Empty")]
    IapCumulativeWrite = 100004,
    /// IAP: Erase operation failed
    #[display("IAP: Erase Operation Failed")]
    IapEraseFailure = 100005,
    /// IAP: Specific command not supported
    #[display("IAP: Specific Command Not Supported")]
    IapCommandNotSupported = 100006,
    /// IAP: Memory access disabled
    #[display("IAP: Memory Access Disabled")]
    IapMemoryAccessDisabled = 100007,

    // EL2Go ProvFW statuses
    /// EL2Go: Device has been successfully provisioned
    #[display("Device has been successfully provisioned.")]
    El2goProvSuccess = 0x5a5a5a5a,

    /// Unknown status code (fallback for unrecognized codes)
    ///
    /// Not defined in any specification, only for handling unexpected or undefined status codes
    #[display("Unknown status code")]
    UnknownStatusCode = 0xdeadbeef,
}

impl From<StatusCode> for u32 {
    /// Convert status code to its numeric representation.
    fn from(value: StatusCode) -> Self {
        value as u32
    }
}
