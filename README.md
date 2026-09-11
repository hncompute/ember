A type-2 hypervisor in Rust with KVM, living in the userspace

## Supported Architecture

- x86-64

## Build

```shell
cd vmm
make
```

The `kvm-box` binary will be placed at `target/release/kvm-box`.

## Preparing the kernel and initramfs

kernel: use pre-compiled and tuned files from firecracker:

```shell
wget https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/kernels/vmlinux.bin
```

initrd:

```shell
git clone https://github.com/marcov/firecracker-initrd.git
cd firecracker-initrd
bash -x ./build.sh
```

## Usage

```shell
$ ./target/release/kvm-box --kernel ./testdata/vmlinux.bin --initrd ./testdata/initrd.img
```

## TODOs

- [ ] GPU slicing (vGPU with virtio-gpu/qemu vhost-user-gpu)

## References
- [kvm-host](https://github.com/sysprog21/kvm-host)
- [virtio-spec-rs](https://github.com/sysprog21/kvm-host)
