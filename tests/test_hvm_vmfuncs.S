/*
 * Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
 * SPDX-License-Identifier: BSD-3-Clause-Clear
 */

/*
 * HVM conformance test: vmfuncs (setvec / setie / getie).
 *
 * Derived from the Hexagon VM specification and the h2 hypervisor
 * kernel/vm/vmfuncs conformance contract:
 *   - vmsetvec: sets the guest event vector base, returns 0 for success.
 *   - vmsetie:  enables/disables guest interrupts, returns the PREVIOUS
 *               interrupt-enable state in r0.
 *   - vmgetie:  returns the current interrupt-enable state (1=enabled,
 *               0=disabled) in r0.
 *
 * No timer or interrupt source is configured, so toggling IE must not
 * deliver any event -- the getie/setie round-trip is fully deterministic.
 */

#include "../hexagon_vm.h"

#define FD_STDOUT 1

.section .start,"awx",@progbits
.global _start
_start:
	// vmsetvec must return 0 (success)
	r0 = ##GUEST_event_vectors
	vmsetvec
	{ p0 = cmp.eq(r0, #0); if (!p0.new) jump:nt _fail }

	// Baseline: interrupts start disabled -> getie == 0
	vmgetie
	{ p0 = cmp.eq(r0, #0); if (!p0.new) jump:nt _fail }

	// Enable interrupts; setie returns PREVIOUS state (disabled -> 0)
	r0 = #1
	vmsetie
	{ p0 = cmp.eq(r0, #0); if (!p0.new) jump:nt _fail }

	// getie now reports enabled (1)
	vmgetie
	{ p0 = cmp.eq(r0, #1); if (!p0.new) jump:nt _fail }

	// Disable interrupts; setie returns PREVIOUS state (enabled -> 1)
	r0 = #0
	vmsetie
	{ p0 = cmp.eq(r0, #1); if (!p0.new) jump:nt _fail }

	// getie now reports disabled (0)
	vmgetie
	{ p0 = cmp.eq(r0, #0); if (!p0.new) jump:nt _fail }

	// All checks passed
	r0 = #5
	r1 = ##pass_args
	trap0(#0)
	r2 = #0
	vmstop

_fail:
	r0 = #5
	r1 = ##fail_args
	trap0(#0)
	r2 = #0xff
	vmstop

	.type	pass_str,@object
pass_str:
	.string	"PASS\n"
	.size	pass_str, 5
	.type	fail_str,@object
fail_str:
	.string	"FAIL\n"
	.size	fail_str, 5
	.type	pass_args,@object
pass_args:
	.word   FD_STDOUT
	.word   pass_str
	.word   5
	.type	fail_args,@object
fail_args:
	.word   FD_STDOUT
	.word   fail_str
	.word   5

.p2align 8
GUEST_event_vectors:
	jump GUEST_event_abort           // 0: reserved
	jump GUEST_event_abort           // 1: machine check
	jump GUEST_event_abort           // 2: general exception
	jump GUEST_event_abort           // 3: reserved
	jump GUEST_event_abort           // 4: reserved
	jump GUEST_event_trap0           // 5: trap0
	jump GUEST_event_abort           // 6: reserved
	jump GUEST_event_abort           // 7: interrupt

GUEST_event_abort:
	r2 = gsr		// return gsr value to shell
	vmstop

GUEST_event_trap0:
	vmrte

.section ".note.ABI-tag", "a"
.align 4
.long 1f - 0f
.long 3f - 2f
.long  1
0:     .asciz "GNU"
1:     .align 4
2:     .long 0
       .long 3,0,0
3:     .align 4
