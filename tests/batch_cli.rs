// Copyright 2026 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

//! Command-line discovery and argument validation for batch mode.

use std::process::Command;

#[test]
fn batch_help_describes_single_session_execution() {
    let output = Command::new(env!("CARGO_BIN_EXE_rblhost"))
        .args(["batch", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("COMMAND_FILE"));
    assert!(stdout.contains("shared device connection"));
    assert!(stdout.contains("Stops at the first error"));
}

#[test]
fn batch_requires_a_command_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_rblhost"))
        .arg("batch")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("<COMMAND_FILE>"));
}

#[test]
fn batch_requires_a_device_like_standalone_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_rblhost"))
        .args(["batch", "commands.txt"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("No device specified")
    );
}
