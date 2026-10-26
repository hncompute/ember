use serde::{Deserialize, Serialize};
use serde_kv_derive::FromKeyValues;
use std::path::PathBuf;

/// Possible types of serial devices
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SerialType {
    /// Move data into the void
    Sink,
    /// Write to standard output
    Stdout,
}

impl Default for SerialType {
    fn default() -> Self {
        Self::Sink
    }
}

/// Serial device hardware types
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SerialHardware {
    /// Standard PC style (8250/16550 compatible) UART (still applied today yall)
    Serial,
}

impl Default for SerialHardware {
    fn default() -> Self {
        Self::Serial
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FromKeyValues)]
#[serde(deny_unknown_fields, rename_all = "kebab-case", default)]
pub struct SerialParameters {
    #[serde(rename = "type")]
    pub type_: SerialType,
    pub hardware: SerialHardware,
    pub name: Option<String>,
    pub path: Option<PathBuf>,
    pub input: Option<PathBuf>,
    #[serde(default = "serial_parameters_default_num")]
    pub num: u8,
    pub console: bool,
    pub stdin: bool,
}

fn serial_parameters_default_num() -> u8 {
    1
}

impl SerialParameters {}
