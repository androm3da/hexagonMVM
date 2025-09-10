/*
 * Test program that expects 5 periodic timer interrupts
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "../hexagon_vm.h"
#include "../dspss.h"

#define PERM_BITS		(0x7 << HVM_PTE_R_BIT)    // X:1 W:1 R:1
#define CACHE_BITS		(0x7 << HVM_PTE_CCC_OFF)
#define GUEST_PTE_BITS		(PERM_BITS | CACHE_BITS | HVM_PTE_PGSIZE_4M)

#define FOUR_MB_BITS		22
#define FOUR_MB			(1 << FOUR_MB_BITS)
#define FOUR_MB_MASK		(~(FOUR_MB - 1))

#define EXPECTED_INTERRUPTS	5

#define CREATE_PTE_ENTRIES(START, END, BITS) \
	/* Number of entries between START and END */ \
	r26 = #(((END-START)+FOUR_MB-1)>>FOUR_MB_BITS); \
	r1 = #(START); \
	r1 = and(r1, #FOUR_MB_MASK); \
	r2 = lsr(r1, #FOUR_MB_BITS); \
	r0 = addasl(r24, r2, #2);	/* r0 = address of first PTE */ \
	r1 = add(r1, #(BITS));		/* r1 = 4MB PTE for the first entry */ \
	r2 = #FOUR_MB;			/* PTE increment */ \
	loop0(1f, r26); \
1: \
	memw(r0 ++ #4) = r1; \
	{ r1 = add(r1, r2) } :endloop0

#define FD_STDOUT                1

.section .start,"awx",@progbits
.global _start
_start:
	r29 = ##(stack + 32*4 - 8)
	r0 = #0xdeadbeef
	r1 = #0xcafebabe
	memd(r29) = r1:0

	// Initialize interrupt counter
	r0 = #0
	memw(##timer_interrupt_count) = r0

	// Get the physical address of guest_pt
	r25 = pc
	r24 = ##(guest_pt-_start)
	r24 = add(r24, r25)

	CREATE_PTE_ENTRIES(_start, _end_guest, GUEST_PTE_BITS)

	r0 = r24
	vmnewmap

	// Install our event vectors
	r0 = ##GUEST_event_vectors
	vmsetvec

	// Enable interrupts globally and locally for timer interrupt (interrupt 4)
	r0 = #1
	vmsetie
	vmgetie
	{ p0 = cmp.eq(r0, #1); if (!p0.new) jump:nt _fail }

	// Enable timer interrupt (interrupt TIMER_INT_NUM) globally and locally
	r0 = #HVM_INTOP_GLOBEN
	r1 = #TIMER_INT_NUM  // Timer interrupt number from dspss.h
	vmintop
	{ p0 = cmp.eq(r0, #0); if (!p0.new) jump:nt _fail }

	r0 = #HVM_INTOP_LOCEN
	r1 = #TIMER_INT_NUM  // Timer interrupt number from dspss.h
	vmintop
	{ p0 = cmp.eq(r0, #0); if (!p0.new) jump:nt _fail }

	// Main loop: wait for EXPECTED_INTERRUPTS timer interrupts
wait_loop:
	// Check if we've received enough interrupts
	r0 = memw(##timer_interrupt_count)
	{ p0 = cmp.eq(r0, #EXPECTED_INTERRUPTS); if (p0.new) jump:nt test_passed }

	// Yield and continue waiting
	vmyield
	jump wait_loop

test_passed:
	// Print the PASS message
	r0 = #5
	r1 = ##pass_args
	trap0(#0)

	// Successful test
	r2 = #0
	vmstop

_fail:
	// Print the FAIL message
	r0 = #5
	r1 = ##fail_args
	trap0(#0)

	r2 = #0xff		// return failure to shell
	vmstop

	.type	pass_str,@object
pass_str:
	.string	"PASS: Received 5 periodic timer interrupts\n"
	.size	pass_str, . - pass_str
	.type	fail_str,@object
fail_str:
	.string	"FAIL: Timer interrupt test failed\n"
	.size	fail_str, . - fail_str
	.type	pass_args,@object
pass_args:
	.word   FD_STDOUT
	.word   pass_str
	.word   . - pass_str
	.type	fail_args,@object
fail_args:
	.word   FD_STDOUT
	.word   fail_str
	.word   . - fail_str

GUEST_event_vectors:
	jump GUEST_event_abort           // 0: reserved
	jump GUEST_event_abort           // 1: machine check
	jump GUEST_event_abort           // 2: general exception
	jump GUEST_event_abort           // 3: reserved
	jump GUEST_event_abort           // 4: reserved
	jump GUEST_event_trap0           // 5: trap0
	jump GUEST_event_abort           // 6: reserved
	jump GUEST_event_interrupt       // 7: interrupt

GUEST_event_abort:
	r2 = gsr		// return gsr value to shell
	vmstop

// Simple trap0 handler for output
//    Cause code 0 is an angel call - do nothing
//    Cause code 1 will exit (r0 has return code to shell)
GUEST_event_trap0:
	r10 = gsr
	r10 = zxth(r10)
	{
		p0 = cmp.eq(r10, #1)
		if (!p0.new) jump:nt 1f
	}
	r2 = r0
	vmstop
1:
	vmrte

GUEST_event_interrupt:
	allocframe(#8)
	memd(r29) = r1:0
	r1 = gsr
	r1 = extractu(r1, #4, #0)  // Extract interrupt number

	// Check if it's the timer interrupt (interrupt TIMER_INT_NUM)
	{ p0 = cmp.eq(r1, #TIMER_INT_NUM); if (!p0.new) jump:nt skip_timer }

	// Increment timer interrupt counter
	r0 = ##timer_interrupt_count
	memw(r0) += #1

skip_timer:
	// Re-enable the interrupt (r1 has the number)
	r0 = #HVM_INTOP_GLOBEN
	vmintop
	r1:0 = memd(r29)
	deallocframe
	vmrte

	.align 4
timer_interrupt_count:
	.word 0

	.align 8
stack:
	.fill 32, 4, 0

#define XLAT4M(VAL) \
	.word (VAL | HVM_PTE_PGSIZE_INVALID);

#define XLAT16M(VAL) \
	XLAT4M(VAL) \
	XLAT4M(VAL | 0x00400000) \
	XLAT4M(VAL | 0x00800000) \
	XLAT4M(VAL | 0x00c00000)

#define XLAT64M(VAL) \
	XLAT16M(VAL) \
	XLAT16M(VAL | 0x01000000) \
	XLAT16M(VAL | 0x02000000) \
	XLAT16M(VAL | 0x03000000)

#define XLAT256M(VAL) \
	XLAT64M(VAL) \
	XLAT64M(VAL | 0x04000000) \
	XLAT64M(VAL | 0x08000000) \
	XLAT64M(VAL | 0x0c000000)

	.p2align 12
guest_pt:
	XLAT256M(0x00000000)
	XLAT256M(0x10000000)
	XLAT256M(0x20000000)
	XLAT256M(0x30000000)
	XLAT256M(0x40000000)
	XLAT256M(0x50000000)
	XLAT256M(0x60000000)
	XLAT256M(0x70000000)
	XLAT256M(0x80000000)
	XLAT256M(0x90000000)
	XLAT256M(0xa0000000)
	XLAT256M(0xb0000000)
	XLAT256M(0xc0000000)
	XLAT256M(0xd0000000)
	XLAT256M(0xe0000000)
	XLAT256M(0xf0000000)
_end_guest:

.section ".note.ABI-tag", "a"
.align 4
.long 1f - 0f          /* name length */
.long 3f - 2f          /* data length */
.long  1               /* note type */

/*
 * vendor name seems like this should be MUSL but lldb doesn't agree.
 */
0:     .asciz "GNU"
1:     .align 4
2:     .long 0 /* linux */
       .long 3,0,0
3:     .align 4
