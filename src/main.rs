//! CLI entry point.
//!
//! Exit codes:
//!   0 — success
//!   1 — parse error (bad JSON)
//!   2 — usage error or I/O error

use std::io::{self, Read, Write};
use std::process::ExitCode;

use json2json::{parse, to_string_compact, to_string_with_indent};

const USAGE: &str = "\
json2json — lossless JSON to JSON transformer

Reads JSON, writes the same JSON back. Key order, duplicate keys and
number lexical forms survive untouched; only whitespace is normalized.

Usage: json2json [OPTIONS] [INPUT] [OUTPUT]

Arguments:
  [INPUT]   Input file; '-' or omitted means stdin
  [OUTPUT]  Output file; '-' or omitted means stdout

Options:
  -c, --compact       One line, no whitespace
  -i, --indent <N>    Spaces per level for pretty output (default: 2)
  -h, --help          Print help
  -V, --version       Print version
";

struct Options {
    compact: bool,
    indent: usize,
    input: Option<String>,
    output: Option<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(msg)) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(CliError::Io(msg)) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
        Err(CliError::Parse(msg)) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

enum CliError {
    Usage(String),
    Io(String),
    Parse(String),
}

fn run() -> Result<(), CliError> {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(Some(o)) => o,
        Ok(None) => return Ok(()), // --help / --version already printed
        Err(e) => return Err(e),
    };

    let input = read_input(options.input.as_deref())?;
    let value = parse(&input).map_err(|e| CliError::Parse(e.to_string()))?;

    let rendered = if options.compact {
        to_string_compact(&value)
    } else {
        to_string_with_indent(&value, options.indent)
    };

    write_output(options.output.as_deref(), &rendered)
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Options>, CliError> {
    let mut options = Options {
        compact: false,
        indent: 2,
        input: None,
        output: None,
    };
    let mut positional: Vec<String> = Vec::new();
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("json2json {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-c" | "--compact" => options.compact = true,
            "-i" | "--indent" => {
                let raw = args
                    .next()
                    .ok_or_else(|| CliError::Usage("--indent requires a value".into()))?;
                options.indent = raw
                    .parse()
                    .map_err(|_| CliError::Usage(format!("invalid indent: {raw:?}")))?;
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(CliError::Usage(format!("unknown option: {arg}")));
            }
            _ => positional.push(arg),
        }
    }

    match positional.len() {
        0 => {}
        1 => options.input = Some(positional.remove(0)),
        2 => {
            options.input = Some(positional.remove(0));
            options.output = Some(positional.remove(0));
        }
        n => {
            return Err(CliError::Usage(format!(
                "expected at most 2 file arguments, got {n}"
            )))
        }
    }
    Ok(Some(options))
}

fn read_input(path: Option<&str>) -> Result<String, CliError> {
    match path {
        None | Some("-") => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| CliError::Io(format!("reading stdin: {e}")))?;
            Ok(buf)
        }
        Some(p) => {
            std::fs::read_to_string(p).map_err(|e| CliError::Io(format!("reading {p}: {e}")))
        }
    }
}

fn write_output(path: Option<&str>, data: &str) -> Result<(), CliError> {
    match path {
        None | Some("-") => {
            let stdout = io::stdout();
            let mut lock = stdout.lock();
            lock.write_all(data.as_bytes())
                .and_then(|()| lock.write_all(b"\n"))
                .map_err(|e| CliError::Io(format!("writing stdout: {e}")))
        }
        Some(p) => std::fs::write(p, data).map_err(|e| CliError::Io(format!("writing {p}: {e}"))),
    }
}
