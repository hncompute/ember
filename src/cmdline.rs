use devices::SerialParameters;
use hypervisor::HypervisorKind;
use remain::sorted;
use std::path::PathBuf;

use argh::FromArgs;

use crate::config::{CpuOptions, Executable, IrqChipKind, MemOptions, parse_serial_options};

#[derive(FromArgs)]
/// ember
pub struct CmdlineArgs {
    #[argh(subcommand)]
    pub command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
pub enum Command {
    // Resume(ResumeCommand),
    Run(RunCommand),
    // Stop(StopCommand),
    // Suspend(SuspendCommand),
}

#[sorted]
#[derive(FromArgs, Default)]
#[argh(subcommand, name = "run", description = "Start a new ember instance")]
pub struct RunCommand {
    #[argh(option, arg_name = "PATH")]
    /// path to user provided ACPI table
    pub acpi_table: Vec<PathBuf>,
    #[argh(option, short = 'c')]
    /// cpu parameters.
    /// Possible key values:
    ///     num-cores=NUM - number of vCPUs. (default: 1)
    pub cpus: Option<CpuOptions>,
    /// hypervisor backend
    #[argh(option)]
    pub hypervisor: Option<HypervisorKind>,
    #[argh(option, short = 'i', arg_name = "PATH")]
    /// temporary root filesystem
    pub initramfs: Option<PathBuf>,
    #[argh(option, arg_name = "kernel")]
    /// type of interrupt controller emulation. "split" is only available for x86 KVM.
    /// TODO: What is IRQ chip split? Go with kernel by default
    pub irqchip: Option<IrqChipKind>,
    #[argh(positional, arg_name = "KERNEL")]
    /// bzImage? of kernel to run
    pub kernel: Option<PathBuf>,
    #[argh(option, short = 'm', arg_name = "N")]
    /// memory parameters.
    /// Possible key values:
    ///     size=NUM - amount of guest memory in MiB. (default: 256)
    pub mem: Option<MemOptions>,
    #[argh(option)]
    /// optional name for the VM.
    pub name: Option<String>,
    #[argh(
        option,
        arg_name = "type=TYPE,[hardware=HW,num=NUM,path=PATH,input=PATH,console,earlycon,stdin]",
        from_str_fn(parse_serial_options)
    )]
    /// comma separated key=value pairs for setting up serial
    /// devices. Can be given more than once.
    /// Possible key values:
    ///     type=(stdout,syslog,sink,file) - Where to route the
    ///        serial device.
    ///        Platform-specific options:
    ///        On Unix: 'unix' (datagram) and 'unix-stream' (stream)
    ///        On Windows: 'namedpipe'
    ///     hardware=(serial) - Which type of
    ///        serial hardware to emulate. Defaults to 8250 UART
    ///        (serial).
    ///     num=(1,2,3,4) - Serial Device Number. If not provided,
    ///        num will default to 1.
    ///     path=PATH - The path to the file to write to when
    ///        type=file
    ///     input=PATH - The path to the file to read from when not
    ///        stdin
    ///     input-unix-stream - (Unix-only) Whether to use the given
    ///        Unix stream socket for input as well as output.
    ///        This flag is only valid when type=unix-stream and
    ///        the socket path is specified with path=.
    ///        Can't be passed when input is specified.
    ///     console - Use this serial device as the guest console.
    ///        Will default to first serial port if not provided.
    ///     earlycon - Use this serial device as the early console.
    ///        Can only be given once.
    ///     stdin - Direct standard input to this serial device.
    ///        Can only be given once. Will default to first serial
    ///        port if not provided.
    pub serial: Vec<SerialParameters>,
}

impl TryFrom<RunCommand> for super::config::VmConfig {
    type Error = String;

    fn try_from(cmd: RunCommand) -> Result<Self, Self::Error> {
        let mut cfg = Self::default();
        if let Some(p) = cmd.kernel {
            cfg.executable_path = Some(Executable::Kernel(p));
        }

        cfg.hypervisor = cmd.hypervisor;

        // `--cpu` parameters
        {
            let cpus = cmd.cpus.unwrap_or_default();
            cfg.vcpu_count = cpus.num_cores;
            cfg.boot_cpu = cpus.boot_cpu.unwrap_or_default();
        }

        let mem = cmd.mem.unwrap_or_default();
        cfg.memory = mem.size;

        for serial_params in cmd.serial {
            let num = serial_params.num;
            let key = (serial_params.hardware, num);

            if cfg.serial_params.contains_key(&key) {
                return Err(format!(
                    "serial hardware {} num {}",
                    serial_params.hardware, num,
                ));
            }

            if serial_params.earlycon {
                // Atm only Serial supports earlycon
                for params in cfg.serial_params.values() {
                    if params.earlycon {
                        return Err(format!(
                            "{} device {} already set as earlycon",
                            params.hardware, params.num,
                        ));
                    }
                }
            }

            if serial_params.stdin {
                if let Some(previous_stdin) = cfg.serial_params.values().find(|sp| sp.stdin) {
                    return Err(format!(
                        "{} device {} already connected to standard input",
                        previous_stdin.hardware, previous_stdin.num,
                    ));
                }
            }
        }

        cfg.irq_chip = cmd.irqchip;
        cfg.initramfs_path = cmd.initramfs;
        cfg.acpi_tables = cmd.acpi_table;
        // TODO: Things to add after runnable: coiommu, seccomp, protection mode/type, swap_dir,
        // disk, PCI cfg + stub PCI device, file_backed_mapping,
        cfg.name = cmd.name;

        // TODO: Add validate_config()

        Ok(cfg)
    }
}
