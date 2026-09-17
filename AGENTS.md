## Code Review Rules

### Hypervisor safety

- Flag unsafe code when its safety invariants are missing or incorrect.
- Flag incorrect ownership or cleanup of file descriptors, memory mappings, and KVM resources.

### KVM and guest memory

- Flag incorrect ioctl arguments and ABI layout assumptions.
- Flag guest-memory ranges that can overflow, overlap, or escape registered memory slots.

### Concurrency and devices

- Flag races, deadlocks, lost events, and incorrect eventfd lifecycle handling.
- Focus on consequential correctness and security problems.
- Do not report formatting or style preferences.
