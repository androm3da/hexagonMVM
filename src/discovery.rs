/*
 * Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
 * SPDX-License-Identifier: BSD-3-Clause-Clear
 */

//! Runtime platform discovery.
//!
//! Device locations and core capabilities are read from the hardware at
//! boot rather than baked in at build time:
//!
//! * `cfgbase` holds bits [35:16] of the physical address of the config
//!   table ROM. The table's `subsystem_base` entry locates the CSR block,
//!   from which the L2VIC and QTIMER bases are derived at fixed offsets.
//! * `rev` identifies the core revision, which gates architectural
//!   features (DM0, VWCTRL, maximum page size).
//!
//! This mirrors what the h2 hypervisor derives from the same table (see
//! `kernel/traps/info/info.ref.c` and `kernel/util/hw/cfg_table.h`), but
//! replaces its build-time `ARCHV`/base-address defines.

use minivm_types::arch::ArchVersion;

/// Config table entries, as byte offsets into the ROM (h2 names these
/// `CFG_TABLE_*` in `kernel/util/max/max.h`).
mod cfg {
    pub const L2TCM_BASE: u32 = 0x00;
    pub const SSBASE: u32 = 0x08;
    pub const JTLB_SIZE_ENTRIES: u32 = 0x2c;
    pub const VTCM_BASE: u32 = 0x38;
    pub const VTCM_SIZE_KB: u32 = 0x3c;
    pub const THREAD_ENABLE_MASK: u32 = 0x48;
    /// Config table base addresses store bits [35:16].
    pub const ADDR_SHIFT: u32 = 16;
}

/// Offsets of devices within the subsystem (CSR) block, for v5 and later.
const L2VIC_OFFSET: u32 = 0x10000;
const TIMER_OFFSET: u32 = 0x20000;
/// The QTIMER view the monitor programs lives in frame 1.
const TIMER_FRAME1_OFFSET: u32 = 0x1000;

/// Everything discovered about the platform at boot.
#[derive(Clone, Copy)]
pub struct Platform {
    /// Physical address of the config table ROM (0 if `cfgbase` is unset).
    pub cfg_base: u32,
    /// Raw `rev` register contents.
    pub rev: u32,
    /// Architecture version decoded from `rev`.
    pub arch: Option<ArchVersion>,
    /// Base of the subsystem CSR block.
    pub ss_base: u32,
    /// L2VIC interrupt controller base.
    pub l2vic_base: u32,
    /// QTIMER frame 1 base.
    pub qtimer_base: u32,
    /// Number of JTLB entries.
    pub jtlb_entries: u32,
    /// Mask of hardware threads present.
    pub thread_mask: u32,
    /// L2TCM base (0 if absent).
    pub l2tcm_base: u32,
    /// VTCM base (0 if absent).
    pub vtcm_base: u32,
    /// VTCM size in KiB (0 if absent).
    pub vtcm_size_kb: u32,
}

impl Platform {
    pub const fn empty() -> Self {
        Self {
            cfg_base: 0,
            rev: 0,
            arch: None,
            ss_base: 0,
            l2vic_base: 0,
            qtimer_base: 0,
            jtlb_entries: 0,
            thread_mask: 0,
            l2tcm_base: 0,
            vtcm_base: 0,
            vtcm_size_kb: 0,
        }
    }

}

#[cfg(target_arch = "hexagon")]
fn read_cfgbase() -> u32 {
    let val: u32;
    unsafe {
        core::arch::asm!("{v} = cfgbase", v = out(reg) val, options(nomem, nostack));
    }
    val
}

#[cfg(target_arch = "hexagon")]
fn read_rev() -> u32 {
    let val: u32;
    unsafe {
        core::arch::asm!("{v} = rev", v = out(reg) val, options(nomem, nostack));
    }
    val
}

/// Read one config table entry. `offset` is a byte offset into the ROM.
#[cfg(target_arch = "hexagon")]
fn cfg_word(cfg_base: u32, offset: u32) -> u32 {
    unsafe { core::ptr::read_volatile((cfg_base + offset) as *const u32) }
}

/// Probe the core and its config table.
#[cfg(target_arch = "hexagon")]
pub fn discover() -> Platform {
    let mut p = Platform::empty();

    p.rev = read_rev();
    p.arch = ArchVersion::from_rev((p.rev & 0xFF) as u8);

    // cfgbase holds bits [35:16]; the low 16 bits of the address are zero.
    let cfgbase = read_cfgbase();
    if cfgbase == 0 {
        return p;
    }
    p.cfg_base = cfgbase << cfg::ADDR_SHIFT;

    let ss = cfg_word(p.cfg_base, cfg::SSBASE);
    if ss != 0 {
        p.ss_base = ss << cfg::ADDR_SHIFT;
        p.l2vic_base = p.ss_base + L2VIC_OFFSET;
        p.qtimer_base = p.ss_base + TIMER_OFFSET + TIMER_FRAME1_OFFSET;
    }

    p.jtlb_entries = cfg_word(p.cfg_base, cfg::JTLB_SIZE_ENTRIES);
    p.thread_mask = cfg_word(p.cfg_base, cfg::THREAD_ENABLE_MASK);
    p.l2tcm_base = cfg_word(p.cfg_base, cfg::L2TCM_BASE) << cfg::ADDR_SHIFT;
    p.vtcm_base = cfg_word(p.cfg_base, cfg::VTCM_BASE) << cfg::ADDR_SHIFT;
    p.vtcm_size_kb = cfg_word(p.cfg_base, cfg::VTCM_SIZE_KB);

    p
}

#[cfg(not(target_arch = "hexagon"))]
pub fn discover() -> Platform {
    Platform::empty()
}
