// Copyright 2026 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

//! Batch parsing, shared-session execution and failure regression tests.

use std::{cell::RefCell, collections::VecDeque, io::Write, rc::Rc, time::Duration};

use clap::{FromArgMatches, Parser};
use mboot::{CommunicationError, protocols::Protocol, tags::status::StatusCode};
use tempfile::NamedTempFile;

use super::{BatchCommand, batch_command, split_command_line};
use crate::{Args, Blhost, Commands, run_blhost};

type Response = Result<Vec<u8>, CommunicationError>;

#[derive(Default)]
struct Session {
    opens: usize,
    closes: usize,
    packets: Vec<Vec<u8>>,
    responses: VecDeque<Response>,
}

struct MockProtocol(Rc<RefCell<Session>>);

impl MockProtocol {
    fn open(session: &Rc<RefCell<Session>>) -> Self {
        session.borrow_mut().opens += 1;
        Self(Rc::clone(session))
    }
}

impl Drop for MockProtocol {
    fn drop(&mut self) {
        self.0.borrow_mut().closes += 1;
    }
}

impl Protocol for MockProtocol {
    fn get_timeout(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn get_polling_interval(&self) -> Duration {
        Duration::from_millis(1)
    }

    fn get_identifier(&self) -> &'static str {
        "batch-test"
    }

    fn read(&mut self, _: usize) -> Response {
        panic!("Unexpected raw read")
    }

    fn write_packet_raw(&mut self, data: &[u8]) -> Result<(), CommunicationError> {
        self.0.borrow_mut().packets.push(data.to_vec());
        Ok(())
    }

    fn read_packet_raw(&mut self, code: u8) -> Response {
        assert_eq!(code, 0xA4);
        self.0
            .borrow_mut()
            .responses
            .pop_front()
            .expect("Unexpected extra command/response")
    }
}

fn response(tag: u8, status: StatusCode, params: &[u32]) -> Vec<u8> {
    let mut data = vec![tag, 0, 0, u8::try_from(params.len() + 1).unwrap()];
    data.extend(u32::from(status).to_le_bytes());
    data.extend(params.iter().flat_map(|value| value.to_le_bytes()));
    data
}

fn property_response() -> Vec<u8> {
    response(0xA7, StatusCode::Success, &[0x4B03_0000])
}

fn run(args: Args, responses: Vec<Response>) -> (anyhow::Result<()>, Session) {
    let session = Rc::new(RefCell::new(Session {
        responses: responses.into(),
        ..Session::default()
    }));
    let result = {
        let mut blhost = Blhost::new(args, MockProtocol::open(&session));
        run_blhost(&mut blhost)
    };
    let session = Rc::try_unwrap(session)
        .ok()
        .expect("Session was not released")
        .into_inner();
    assert_eq!(session.opens, 1);
    assert_eq!(session.closes, 1);
    (result, session)
}

fn run_script(script: &[u8], responses: Vec<Response>) -> (anyhow::Result<()>, Session) {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(script).unwrap();
    let args = Args::try_parse_from([
        "rblhost",
        "--silent",
        "--port",
        "mock",
        "batch",
        file.path().to_str().unwrap(),
    ])
    .unwrap();
    run(args, responses)
}

#[test]
fn executes_in_order_with_one_connection_and_fresh_argument_defaults() {
    let script = b"# header\r\n\r\nget-property 1 7 # first\r\nget-property 1\r\nget-property 1 2";
    let (result, session) = run_script(
        script,
        vec![
            Ok(property_response()),
            Ok(property_response()),
            Ok(property_response()),
        ],
    );
    result.unwrap();
    assert!(session.responses.is_empty());
    assert_eq!(session.packets.len(), 3);
    for (packet, index) in session.packets.iter().zip([7u32, 0, 2]) {
        assert_eq!(packet[6], 7);
        assert_eq!(&packet[10..14], &1u32.to_le_bytes());
        assert_eq!(&packet[14..18], &index.to_le_bytes());
    }
}

#[test]
fn empty_and_comment_only_batches_send_nothing() {
    for script in [b"".as_slice(), b" \r\n# comment\n  # comment\n"] {
        let (result, session) = run_script(script, vec![]);
        result.unwrap();
        assert_eq!(session.packets, Vec::<Vec<u8>>::new());
    }
}

#[test]
fn parses_quotes_escapes_literal_hashes_and_command_options() {
    let mut parser = batch_command();
    for (line, expected) in [
        (
            r#"read-memory 0 4 "memory #1.bin" --use-hexdump # comment"#,
            "memory #1.bin",
        ),
        (
            r"read-memory 0 4 'C:\images\memory dump.bin'",
            r"C:\images\memory dump.bin",
        ),
        (r"read-memory 0 4 memory\ dump.bin", "memory dump.bin"),
        (r#"read-memory 0 4 "" "#, ""),
    ] {
        let words = split_command_line(line).unwrap();
        let matches = parser
            .try_get_matches_from_mut(std::iter::once("batch").chain(words.iter().map(String::as_str)))
            .unwrap();
        let command = BatchCommand::from_arg_matches(&matches).unwrap().command;
        match command {
            Commands::ReadMemory { file, .. } => assert_eq!(file.as_deref(), Some(expected)),
            other => panic!("Expected read-memory, got {other:?}"),
        }
    }
}

#[test]
fn supports_existing_nested_command_parser() {
    let parsed = BatchCommand::try_parse_from(["batch", "key-provisioning", "enroll"]).unwrap();
    assert!(matches!(parsed.command, Commands::KeyProvisioning(_)));
}

#[test]
fn parse_errors_stop_at_the_source_line() {
    for (line, name) in [
        ("unknown-command", "unknown-command"),
        ("get-property", "get-property"),
        ("get-property not-a-property", "get-property"),
        ("get-property 1 --port other", "get-property"),
        ("--port other get-property 1", "--port"),
        ("batch another.txt", "batch"),
        ("list-usb", "list-usb"),
        ("get-property --help", "get-property"),
        ("read-memory 0 4 'unterminated", "read-memory"),
        ("get-property 1\\", "get-property"),
    ] {
        let script = format!("# header\nget-property 1\n{line}\nget-property 1\n");
        let (result, session) = run_script(script.as_bytes(), vec![Ok(property_response())]);
        let error = format!("{:#}", result.unwrap_err());
        assert!(error.contains(&format!(":3 ({name})")), "{error}");
        assert_eq!(session.packets.len(), 1, "{line}");
    }
}

#[test]
fn communication_error_stops_execution_and_closes_session() {
    let (result, session) = run_script(
        b"get-property 1\nget-property 1\nget-property 1",
        vec![Ok(property_response()), Err(CommunicationError::Timeout)],
    );
    let error = result.unwrap_err();
    assert!(error.to_string().contains(":2 (get-property)"));
    assert!(matches!(error.downcast_ref(), Some(CommunicationError::Timeout)));
    assert_eq!(session.packets.len(), 2);
}

#[test]
fn unsuccessful_device_status_stops_execution() {
    let (result, session) = run_script(
        b"get-property 1\nget-property 1",
        vec![Ok(response(0xA7, StatusCode::Fail, &[0]))],
    );
    let error = result.unwrap_err();
    assert!(error.to_string().contains(":1 (get-property)"));
    assert!(matches!(
        error.downcast_ref(),
        Some(CommunicationError::UnexpectedStatus(StatusCode::Fail, _))
    ));
    assert_eq!(session.packets.len(), 1);
}

fn otp_verify_responses() -> Vec<Response> {
    vec![
        Ok(response(0xA0, StatusCode::Success, &[0x0E])),
        Ok(response(0xAF, StatusCode::Success, &[4, 0])),
    ]
}

#[test]
fn otp_verify_failure_is_fatal_in_batch_but_standalone_behavior_is_unchanged() {
    for command in ["flash-program-once 1 4 1 --verify", "efuse-program-once 1 1 --verify"] {
        let script = format!("{command}\nget-property 1");
        let (result, session) = run_script(script.as_bytes(), otp_verify_responses());
        let error = result.unwrap_err();
        assert!(error.to_string().contains(":1 ("));
        assert!(matches!(
            error.downcast_ref(),
            Some(CommunicationError::UnexpectedStatus(StatusCode::OtpVerifyFail, _))
        ));
        assert_eq!(session.packets.len(), 2);

        let args = Args::try_parse_from(
            ["rblhost", "--port", "mock"]
                .into_iter()
                .chain(command.split_whitespace()),
        )
        .unwrap();
        let (result, session) = run(args, otp_verify_responses());
        result.unwrap();
        assert_eq!(session.packets.len(), 2);
    }
}

#[test]
fn invalid_utf8_is_reported_with_line_number() {
    let (result, session) = run_script(b"get-property 1\n\xff\nget-property 1", vec![Ok(property_response())]);
    assert!(result.unwrap_err().to_string().contains(":2: cannot read command"));
    assert_eq!(session.packets.len(), 1);
}

#[test]
fn missing_batch_file_releases_session() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.txt");
    let args = Args::try_parse_from(["rblhost", "--port", "mock", "batch", missing.to_str().unwrap()]).unwrap();
    let (result, session) = run(args, vec![]);
    assert!(result.unwrap_err().to_string().contains("Cannot open batch file"));
    assert_eq!(session.packets, Vec::<Vec<u8>>::new());
}

#[test]
fn command_file_errors_stop_batch() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.bin");
    let script = format!("get-property 1\nload-image '{}'\nget-property 1", missing.display());
    let (result, session) = run_script(script.as_bytes(), vec![Ok(property_response())]);
    assert!(result.unwrap_err().to_string().contains(":2 (load-image)"));
    assert_eq!(session.packets.len(), 1);
}

#[test]
fn supports_adjacent_comments_and_preserves_quoted_hashes() {
    let (result, session) = run_script(b"get-property 1#comment", vec![Ok(property_response())]);
    result.unwrap();
    assert_eq!(session.packets.len(), 1);
    for (line, expected) in [
        (r##"load-image "file#name.bin"#comment"##, "file#name.bin"),
        (r"load-image 'file#name.bin'#comment", "file#name.bin"),
        (r"load-image file\#name.bin#comment", "file#name.bin"),
        (r##"load-image "file\"#name.bin"#comment"##, "file\"#name.bin"),
        (r"load-image 'file\'#comment", "file\\"),
        ("load-image 'f\u{ed}le#name.bin'#comment", "f\u{ed}le#name.bin"),
    ] {
        assert_eq!(split_command_line(line).unwrap()[1], expected);
    }
}
