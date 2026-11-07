use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use acpi_tables::sdt::SDT;
use anyhow::Context;
use anyhow::Result;
use arch::{VmComponents, VmImage};
use base::debug;
use devices::{SerialHardware, SerialParameters};
use hypervisor::HypervisorKind;
use remain::sorted;
use resources::AddressRange;
use serde::{Deserialize, Serialize};
use serde_keyvalue::FromKeyValues;

const KVM_PATH: &str = "/dev/kvm";

/// Aggregate of all configurable options for a running VM.
#[derive(Serialize, Deserialize)]
#[sorted]
pub struct VmConfig {
    pub acpi_tables: Vec<PathBuf>,
    pub boot_cpu: usize,
    pub executable_path: Option<Executable>,
    pub hugepages: bool,
    pub hypervisor: Option<HypervisorKind>,
    pub initramfs_path: Option<PathBuf>,
    pub irq_chip: Option<IrqChipKind>,
    pub memory: Option<u64>,
    // For system allocator?
    pub mmio_address_ranges: Vec<AddressRange>,
    pub name: Option<String>,
    #[serde(with = "serde_serial_params")]
    pub serial_params: BTreeMap<(SerialHardware, u8), SerialParameters>,
    pub vcpu_count: Option<usize>,
}

impl Default for VmConfig {
    fn default() -> VmConfig {
        VmConfig {
            acpi_tables: Vec::new(),
            boot_cpu: 0,
            executable_path: None,
            hugepages: false,
            hypervisor: None,
            initramfs_path: None,
            irq_chip: None,
            memory: None,
            mmio_address_ranges: Vec::new(),
            name: None,
            serial_params: BTreeMap::new(),
            vcpu_count: None,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize, FromKeyValues)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct CpuOptions {
    /// Number of CPU cores.
    #[serde(default)]
    pub num_cores: Option<usize>,
    /// Select which CPU to boot from.
    #[serde(default)]
    pub boot_cpu: Option<usize>,
}

#[derive(Debug, Default, Deserialize, Serialize, FromKeyValues, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct MemOptions {
    /// Amount of guest memory in MiB.
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Executable {
    /// An executable intended to be run as a BIOS directly.
    Bios(PathBuf),
    /// An ELF Linux kernel.
    Kernel(PathBuf),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExitState {
    Reset,
    Stop,
    Crash,
    GuestPanic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, FromKeyValues)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum IrqChipKind {
    /// All interrupt controllers are emulated in the kernel.
    #[serde(rename_all = "kebab-case")]
    Kernel,
}

pub fn parse_serial_options(s: &str) -> Result<SerialParameters, String> {
    let params: SerialParameters = from_key_values(s)?;

    validate_serial_parameters(&params)?;

    Ok(params)
}

pub fn from_key_values<'a, T: Deserialize<'a>>(value: &'a str) -> Result<T, String> {
    serde_keyvalue::from_key_values(value).map_err(|e| e.to_string())
}

pub fn validate_serial_parameters(params: &SerialParameters) -> Result<(), String> {
    if params.stdin && params.input.is_some() {
        return Err("Cannot specify both stdin and input options".to_string());
    }
    if params.num < 1 {
        return Err(invalid_value_err(
            params.num.to_string(),
            "Serial port num must be at least 1",
        ));
    }

    if params.hardware == SerialHardware::Serial && params.num > 4 {
        return Err(invalid_value_err(
            format!("{}", params.num),
            "Serial port num must be 4 or less",
        ));
    }

    Ok(())
}

pub fn invalid_value_err<T: AsRef<str>, S: ToString>(value: T, expected: S) -> String {
    format!("invalid value {}: {}", value.as_ref(), expected.to_string())
}

mod serde_serial_params {
    use std::collections::BTreeMap;

    use devices::{SerialHardware, SerialParameters};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        params: &BTreeMap<(SerialHardware, u8), SerialParameters>,
        ser: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let v: Vec<(&(SerialHardware, u8), &SerialParameters)> = params.iter().collect();
        serde::Serialize::serialize(&v, ser)
    }

    pub fn deserialize<'a, D>(
        de: D,
    ) -> Result<BTreeMap<(SerialHardware, u8), SerialParameters>, D::Error>
    where
        D: Deserializer<'a>,
    {
        let params: Vec<((SerialHardware, u8), SerialParameters)> =
            serde::Deserialize::deserialize(de)?;
        Ok(BTreeMap::from_iter(params))
    }
}

pub fn run_config(cfg: VmConfig) -> Result<ExitState> {
    let components = setup_vm_components(&cfg)?;

    let hypervisor = cfg
        .hypervisor
        .clone()
        .or_else(|| {
            Some(HypervisorKind::Kvm {
                device: Some(Path::new(KVM_PATH).to_path_buf()),
            })
        })
        .context("no enabled hypervisor")?;

    debug!("creating hypervisor: {:?}", hypervisor);
    match hypervisor {
        HypervisorKind::Kvm { device } => run_kvm(device.as_deref(), cfg, components),
    }
}

fn setup_vm_components(cfg: &VmConfig) -> Result<VmComponents> {
    let acpi_sdts = cfg
        .acpi_tables
        .iter()
        .map(|p| {
            SDT::from_file(p).with_context(|| format!("failed to open ACPI file {}", p.display()))
        })
        .collect::<Result<Vec<SDT>>>()?;
    let initramfs_image = if let Some(initramfs_path) = &cfg.initramfs_path {
        Some(
            open_file_or_duplicate(initramfs_path, OpenOptions::new().read(true)).with_context(
                || format!("failed to open initramfs {}", initramfs_path.display()),
            )?,
        )
    } else {
        None
    };

    let vm_image = match cfg.executable_path {
        Some(Executable::Kernel(ref kernel_path)) => VmImage::Kernel(
            open_file_or_duplicate(kernel_path, OpenOptions::new().read(true)).with_context(
                || format!("failed to open kernel image {}", kernel_path.display()),
            )?,
        ),
        Some(Executable::Bios(ref bios_path)) => VmImage::Bios(
            open_file_or_duplicate(bios_path, OpenOptions::new().read(true))
                .with_context(|| format!("failed to open bios {}", bios_path.display()))?,
        ),
        _ => panic!("did not receive a bios or kernel, should be impossible."),
    };

    let vcpu_count = cfg.vcpu_count.unwrap_or(1);

    Ok(VmComponents {
        acpi_sdts,
        initramfs_image,
        vcpu_count,
        vm_image,
    })
}

fn run_kvm(
    device_path: Option<&Path>,
    cfg: VmConfig,
    components: VmComponents,
) -> Result<ExitState> {
    use devices::KvmKernelIrqChip;
    use hypervisor::kvm::Kvm;
    use hypervisor::kvm::KvmVm;

    let device_path = device_path.unwrap_or(Path::new(KVM_PATH));
    let kvm = Kvm::new_with_path(device_path)
        .with_context(|| format!("failed to open KVM device {}", device_path.display()))?;
}
