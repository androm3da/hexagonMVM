.section .text.device_discovery

// Configuration table structure offsets
.equ CFG_MAGIC,         0x00
.equ CFG_VERSION,       0x04
.equ CFG_NUM_ENTRIES,   0x08
.equ CFG_ENTRY_SIZE,    0x10  // Each entry is 16 bytes
.equ CFG_ENTRY_TYPE,    0x00  // Offset within entry
.equ CFG_ENTRY_ADDR,    0x08  // Offset within entry

// Device types in config table
.equ DEV_TYPE_DSP_SUBSYS, 0x44535053  // "DSPS"

// DSP subsystem offsets
.equ DSP_SUBSYS_L2VIC_OFFSET,  0x00910000  // Fixed offset to L2VIC
.equ DSP_SUBSYS_QTIMER_OFFSET, 0x00920000  // Fixed offset to QTimer

// Global variables to store discovered addresses
.section .data
.global qtimer_base_addr
.global l2vic_base_addr

.extern MINIVM_lock
MINIVM_lock:
    .word 0
qtimer_base_addr:
    .word 0
l2vic_base_addr:
    .word 0

.section .text

// Main device discovery routine
// Uses cfgbase register to find config table and locate devices
find_devices:
    allocframe(#32)
    memd(r29+#0) = r17:16
    memd(r29+#8) = r19:18

    // Read cfgbase register to get config table address
    r16 = cfgbase

    // Get DSP subsystem base address
    r19:18 = memd(r18+#CFG_ENTRY_ADDR)

    // Calculate L2VIC address
    r0 = ##DSP_SUBSYS_L2VIC_OFFSET
    r0 = add(r18, r0)
    r1 = ##l2vic_base_addr
    memw(r1) = r0

    // Map L2VIC via TLB
    r16 = r0  // Save L2VIC address
    call map_device_memory

    // Calculate QTimer address
    r0 = ##DSP_SUBSYS_QTIMER_OFFSET
    r0 = add(r18, r0)
    r1 = ##qtimer_base_addr
    memw(r1) = r0

    // Map QTimer via TLB
    r16 = r0  // Save QTimer address
    call map_device_memory

    // Success
    r0 = #0
    jump discovery_done

discovery_fail:
    r0 = #-1

discovery_done:
    r17:16 = memd(r29+#0)
    r19:18 = memd(r29+#8)
    dealloc_return

// TLB constants
.equ TARGET_PAGE_BITS,  16      // 64K pages
.equ TLB_PGSIZE_64K,    0x02
.equ TLB_C_WB_C,        0x05    // Write-back cacheable
.equ TLB_CCCC_WIDTH,    4
.equ TLB_CCCC_OFF,      2
.equ TLB_XWRU_WIDTH,    4
.equ TLB_XWRU_OFF,      6
.equ TLB_VG_WIDTH,      2
.equ TLB_VG_OFF,        0
.equ TLB_DEVICE_ENTRY,  10      // TLB entry for device mappings

// Map device memory via TLB
// Input: r16 = physical address to map
// Preserves all registers
map_device_memory:
    allocframe(#48)
    memd(r29+#0) = r17:16
    memd(r29+#8) = r19:18
    memd(r29+#16) = r21:20
    memd(r29+#24) = r23:22
    memd(r29+#32) = r29:28
    memd(r29+#40) = r31:30

    // Create TLB entry for device memory
    // r28 = RTE.L (physical page number and attributes)
    r28 = lsr(r16, #TARGET_PAGE_BITS)
    r28 = asl(r28, #1)              // Make room for page size bits
    r28 = or(r28, #TLB_PGSIZE_64K)

    // Device memory attributes (uncached, unbuffered)
    r23 = #0x00                     // Uncached for device memory
    r28 = insert(r23, #TLB_CCCC_WIDTH, #TLB_CCCC_OFF)

    // Access permissions: X:0 W:1 R:1 U:0 (supervisor read/write)
    r23 = #0x6
    r28 = insert(r23, #TLB_XWRU_WIDTH, #TLB_XWRU_OFF)

    // r29 = RTE.H (virtual page number and flags)
    r29 = lsr(r16, #TARGET_PAGE_BITS)  // Use identity mapping

    // Set Valid and Global bits
    r23 = #0x3                      // V:1 G:1
    r29 = insert(r23, #TLB_VG_WIDTH, #TLB_VG_OFF)

    // Acquire TLB lock
    r22 = ##MINIVM_lock
    r21 = #1
1:
    r20 = memw_locked(r22)
    {
        p0 = cmp.eq(r20, #0)        // lock not available
        if (!p0.new) jump:nt 1b     // spin
    }
    memw_locked(r22, p0) = r21      // write 1
    if (!p0) jump 1b

    // Write TLB entry
    r21 = #TLB_DEVICE_ENTRY
    tlbw(r29:28, r21)

    // Release lock
    r20 = #0
    memw(r22) = r20

    // Restore registers
    r17:16 = memd(r29+#0)
    r19:18 = memd(r29+#8)
    r21:20 = memd(r29+#16)
    r23:22 = memd(r29+#24)
    r29:28 = memd(r29+#32)
    r31:30 = memd(r29+#40)
    dealloc_return
