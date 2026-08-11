use log::error;
use std::{env, io, panic, path::PathBuf};
use vmm_sys_util::terminal::Terminal;

#[derive(argh::FromArgs, Debug)] // Procedural macro to implement common traits
#[argh(description = "A simple hypervisor")] // Derive-based arg parser
struct Args {
    #[argh(option, long = "kernel", description = "path to kernel image")]
    kernel: Option<PathBuf>,

    #[argh(option, long = "cmdline", description = "kernel boot cmdline")]
    boot_cmdline: Option<String>,

    #[argh(option, long = "initramfs", description = "path to initramfs")]
    initramfs: Option<PathBuf>,

    #[argh(
        switch,
        short = 'v',
        long = "version",
        description = "print version info"
    )]
    version: bool,
}

fn main() -> anyhow::Result<()> {
    if option_env!("RUST_LOG").is_none() {
        // TODO: Why log?
        // unsafe because nightly?
        unsafe {
            env::set_var("RUST_LOG", "info");
        }
    }

    env_logger::init();

    // Reset terminal to canonical mode(basic editing, press Enter to send commands) if panic occurs
    let stdin = io::stdin();

    // Run after a thread panics but before panic runtime sets in
    panic::set_hook(Box::new(move |info| {
        // This closure takes ownership of data out of its scope
        error!("ember {}", info);

        // NOTE: Lock for whom? thread?
        if let Err(err) = stdin.lock().set_canon_mode() {
            error!(
                "Failure while trying to reset stdin to canonical mode: {}",
                err
            );
        }
    }));

    let args = argh::from_env::<Args>();
    if args.version {
        print_version();
        return Ok(());
    }

    Ok(())
}

fn print_version() {
    println!("ember {}", std::env!("CARGO_PKG_VERSION"));
    println!("{}\n", std::env!("CARGO_PKG_DESCRIPTION"));
    println!("Written by {}", std::env!("CARGO_PKG_AUTHORS"));
}
