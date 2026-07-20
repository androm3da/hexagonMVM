
# Hexagon minivm

A Rust implementation of the
[Hexagon Virtual Machine specification](https://docs.qualcomm.com/bundle/publicresource/80-NB419-3_REV_A_Hexagin_Virtual_Machine_Specification.pdf)
([mirror](https://archive.is/yzlri)).
The Hexagon VM is a type-1 hypervisor and portability layer for Qualcomm
Hexagon DSPs.

## Status

All five guest tests and the on-target self-tests pass under
`qemu-system-hexagon -M virt`. Guest tests exercise individual VM operations
(trap1 calls, TLB management, interrupt delivery, user-mode exceptions).

A Hexagon Linux kernel started through the firmware boot protocol enters the
kernel at `PAGE_OFFSET`, runs early boot, and reaches its first VM calls
(`vmversion`, `vmnewmap`). It stalls there: `vmnewmap` with
`VM_TRANS_TYPE_TABLE` does not yet switch the guest onto page-table
translation, so mappings the kernel installs are not honoured. The page-table
walker itself is implemented and unit-tested
(`crates/minivm-mem/src/translate/table.rs`); wiring `vmnewmap` to allocate an
ASID rooted at the guest table is the remaining work.

## Project structure

The top-level crate (`src/main.rs`) is the `#![no_std]` hypervisor binary,
built for `hexagon-unknown-none-elf`. Subsystem logic lives in workspace
crates under `crates/`, each with host-runnable unit tests:

| Crate | Purpose |
|-------|---------|
| `minivm-types` | Shared constants, register layouts, error codes |
| `minivm-arch` | TLB, cache, and architectural helpers |
| `minivm-mem` | Physical/virtual memory management |
| `minivm-sync` | Spinlocks and synchronization primitives |
| `minivm-sched` | Scheduler data structures (ready/run lists) |
| `minivm-thread` | Thread context and state |
| `minivm-vm` | VM configuration and VMID management |
| `minivm-trap` | Trap decoding and dispatch tables |
| `minivm-event` | Event/interrupt routing |
| `minivm-intc` | Interrupt controller abstraction |
| `minivm-timer` | Timer management |
| `minivm-power` | Power state tracking |
| `minivm-init` | Initialization sequences |
| `minivm-guest-tests` | Integration tests that build and run guest binaries on QEMU |

## Building

### Prerequisites

- Rust nightly (for `-Zbuild-std`)
- `hexagon-unknown-none-elf` target support

### Debug build

```bash
cargo +nightly build -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
```

### Release build

```bash
cargo +nightly build -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem --release
```

## Testing

### Host unit tests

The workspace crates have 248+ unit tests that run on the host:

```bash
cargo test -p minivm-types -p minivm-mem -p minivm-timer -p minivm-init \
    -p minivm-power -p minivm-sched -p minivm-sync -p minivm-trap \
    -p minivm-vm -p minivm-event -p minivm-arch \
    --all-targets --target x86_64-unknown-linux-gnu
```

### Guest integration tests

Guest tests build Hexagon assembly programs and run them on QEMU with minivm
as the kernel. They require `clang` (with Hexagon target), `llvm-objcopy`, and
`qemu-system-hexagon`.

Via cargo (auto-skips when tools are missing):

```bash
cargo test -p minivm-guest-tests --target x86_64-unknown-linux-gnu
```

Via make (for standalone use):

```bash
make guest-tests
```

### Running a guest

Flat guest images are loaded at the guest entry address and run with an
identity mapping:

```bash
qemu-system-hexagon -M virt -bios none -nographic \
    -kernel ./minivm \
    -device "loader,addr=0xa0000000,file=./guest.bin"
```

A Linux-style kernel is booted by running minivm as the machine firmware.
QEMU then loads the kernel separately and passes minivm an FDT pointer,
which selects the Linux boot protocol (entry at `PAGE_OFFSET`, DTB in r0):

```bash
qemu-system-hexagon -M virt -m 4G -nographic \
    -bios ./minivm -kernel ./vmlinux
```

`-bios none` matters for the flat-guest case: without it QEMU loads its
bundled h2 firmware and relocates `-kernel`.

### Debugging

Attach LLDB by starting QEMU with GDB server flags:

```bash
qemu-system-hexagon -M virt -nographic -s -S \
    -kernel ./minivm \
    -device "loader,addr=0xa0000000,file=./vmlinux.bin"
```

Then in another terminal:

```bash
lldb -- -o 'gdb-remote localhost:1234' -o 'break set -a 0x20000000' -o c
```

## License

This project is [licensed](LICENSE) under the BSD 3-clause "Clear" license.
