mod cmdline;
use anyhow::Result;
use argh::FromArgs;
use base::{debug, error};
use cmdline::{CmdlineArgs, RunCommand};

use crate::cmdline::Command;

#[repr(i32)] // Set the size for enum
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Exit codes
enum CommandStatus {
    SuccessOrVmStop = 0,
    InvalidArgs = 35,
}

/// Return true if argument is a flag e.g., -s or --long
fn is_flag(arg: &str) -> bool {
    arg.len() > 1 && arg.starts_with('-')
}

fn prepare_argh_args<I: IntoIterator<Item = String>>(argh_iter: I) -> Vec<String> {
    let mut args: Vec<String> = Vec::default();
    for arg in argh_iter {
        match arg.as_str() {
            "-h" => args.push("--help".to_string()),
            arg if is_flag(arg) => {
                // Split --arg=val to --arg val
                if let Some((key, val)) = arg.split_once('=') {
                    args.push(key.to_string());
                    args.push(val.to_string());
                } else {
                    args.push(arg.to_string());
                }
            }
            arg => args.push(arg.to_string()),
        }
    }

    args
}

fn run_vm(cmd: RunCommand) -> Result<CommandStatus> {
    Ok(CommandStatus::SuccessOrVmStop)
}

fn ember_main<I: IntoIterator<Item = String>>(args: I) -> Result<CommandStatus> {
    let args = prepare_argh_args(std::env::args());
    // Convert String (growable) to &str for readonly
    let args = args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let args = match CmdlineArgs::from_args(&args[..1], &args[1..]) {
        Ok(args) => args,
        Err(e) => {
            error!("arg parsing failed: {}", e.output);
            return Ok(CommandStatus::InvalidArgs);
        }
    };

    let ret = match args.command {
        Command::Run(cmd) => run_vm(cmd),
    };

    ret.map(|_| CommandStatus::SuccessOrVmStop)
}

fn main() {
    debug!("ember started");
    let res = ember_main(std::env::args());
    let exit_code = match &res {
        Ok(code) => *code as i32,
        Err(e) => 1,
    };
    std::process::exit(exit_code);
}
