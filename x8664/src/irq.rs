use anyhow::{Context, Result};
use kvm_bindings::{KVM_PIT_SPEAKER_DUMMY, kvm_pit_config};
use kvm_ioctls::VmFd;

pub fn init_irqchip(vm: &VmFd) -> Result<()> {
    vm.create_irq_chip().context("failed to create irq chip")?;

    let pit_config = kvm_pit_config {
        flags: KVM_PIT_SPEAKER_DUMMY,
        ..Default::default()
    };

    vm.create_pit2(pit_config)
        .context("failed to create pit2")?;

    Ok(())
}
