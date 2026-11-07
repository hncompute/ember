#![cfg(feature = "argh_derive")]

use serde::Deserialize;
use serde_keyvalue::argh::{FromArgValue, FromArgs};
use serde_keyvalue::FromKeyValues;

#[derive(Debug, Deserialize, PartialEq, FromKeyValues)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Options {
    num_threads: usize,
    #[serde(default)]
    enabled: bool,
    memory: Option<Memory>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Memory {
    start: u64,
}

#[derive(FromArgs)]
/// Test key-value argument integration.
struct Command {
    /// worker options
    #[argh(option)]
    worker: Options,
}

#[test]
fn parses_an_argh_option_with_nested_values() {
    let command = Command::from_args(
        &["test"],
        &["--worker", "num-threads=4,enabled,memory=[start=4096]"],
    )
    .unwrap();
    assert_eq!(
        command.worker,
        Options {
            num_threads: 4,
            enabled: true,
            memory: Some(Memory { start: 4096 }),
        }
    );
}

#[test]
fn propagates_deserializer_errors() {
    for input in ["num-threads=invalid", "num-threads=4,typo=true"] {
        let expected = serde_keyvalue::from_key_values::<Options>(input)
            .unwrap_err()
            .to_string();
        assert_eq!(Options::from_arg_value(input), Err(expected));
    }
}
