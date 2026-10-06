//! End-to-end CLI tests, run against the real binary.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_json2json");

fn run(args: &[&str], stdin: &str) -> (bool, String, String) {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn json2json");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn stdin_to_stdout_pretty() {
    let (ok, out, _) = run(&[], r#"{"a":[1,2],"b":null}"#);
    assert!(ok);
    assert_eq!(
        out,
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": null\n}\n"
    );
}

#[test]
fn stdin_to_stdout_compact() {
    let (ok, out, _) = run(&["--compact"], "{ \"a\" : [ 1 , 2 ] }");
    assert!(ok);
    assert_eq!(out, "{\"a\":[1,2]}\n");
}

#[test]
fn dash_works_for_stdin_stdout() {
    let (ok, out, _) = run(&["-", "-"], "[1]");
    assert!(ok);
    assert_eq!(out, "[\n  1\n]\n");
}

#[test]
fn custom_indent() {
    let (ok, out, _) = run(&["--indent", "4"], "{\"k\":true}");
    assert!(ok);
    assert_eq!(out, "{\n    \"k\": true\n}\n");
}

#[test]
fn parse_error_reports_position_and_exit_code() {
    let (ok, _, err) = run(&[], "{\"a\": x}");
    assert!(!ok);
    assert!(err.contains("line 1"), "stderr: {err}");
}

#[test]
fn invalid_input_file_is_io_error() {
    let status = Command::new(BIN)
        .arg("/nonexistent/file.json")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run");
    assert_eq!(status.code(), Some(2));
}

#[test]
fn bad_json_exit_code_is_one() {
    let status = Command::new(BIN)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut c| {
            c.stdin
                .as_mut()
                .unwrap()
                .write_all(b"{oops")
                .and_then(|()| c.wait())
        })
        .expect("run");
    assert_eq!(status.code(), Some(1));
}

#[test]
fn file_to_file_round_trip() {
    let dir = std::env::temp_dir().join("json2json-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.json");
    let output = dir.join("out.json");
    std::fs::write(&input, "{\"b\":1,\"a\":2,\"b\":3}").unwrap();

    let status = Command::new(BIN)
        .args([input.to_str().unwrap(), output.to_str().unwrap()])
        .status()
        .expect("run");
    assert!(status.success());
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        "{\n  \"b\": 1,\n  \"a\": 2,\n  \"b\": 3\n}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn help_and_version() {
    let (ok, out, _) = run(&["--help"], "");
    assert!(ok);
    assert!(out.contains("Usage: json2json"));

    let (ok, out, _) = run(&["--version"], "");
    assert!(ok);
    assert!(out.starts_with("json2json "));
}
