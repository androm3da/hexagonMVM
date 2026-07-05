/*
 * Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
 * SPDX-License-Identifier: BSD-3-Clause-Clear
 */

/*
 * HVM conformance test: illegal vmtrap handling.
 *
 * Derived from the h2 hypervisor kernel/vm/vmtrap conformance contract:
 * only a fixed set of trap1 numbers are valid VM operations; the remaining
 * numbers (6, 7, 8, 9, 12, 23, 27-31) are illegal.  Per the Hexagon VM
 * spec, issuing an illegal vmtrap from guest mode must raise a guest ERROR
 * event through the guest event vector base (GEVB), at the error vector
 * (offset 8 = slot 2, "general exception"), with the "no guest permission"
 * cause (0x1B) reported in GSR[7:0].
 *
 * The test issues trap1(#7) (illegal) and PASSES only if the guest error
 * vector is entered with the expected cause.  If execution instead falls
 * through the illegal trap without an error event, the test FAILs.
 */

#include "../hexagon_vm.h"

#define FD_STDOUT 1
#define HVM_CAUSE_NO_GUEST_PERM 0x1b

.section .start,"awx",@progbits
.global _start
_start:
	r0 = ##GUEST_event_vectors
	vmsetvec

	// Issue an illegal VM trap (trap number 7 is not a valid vmop).
	// A conforming VM delivers an ERROR event to the guest error vector;
	// control must NOT return here.
	trap1(#7)

	// Reached here => the illegal trap did not raise an error event.
	jump _fail

// Guest ERROR vector (slot 2): entered on the illegal trap.
GUEST_event_error:
	// GSR[7:0] carries the fault cause; must be "no guest permission".
	r2 = gsr
	r2 = and(r2, #0xff)
	{ p0 = cmp.eq(r2, #HVM_CAUSE_NO_GUEST_PERM); if (!p0.new) jump:nt _fail }

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
	jump GUEST_event_error           // 2: general exception (error)
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
