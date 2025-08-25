.section .text.l2vic_interface

// L2VIC register offsets
.equ L2VIC_INT_ENABLE,      0x100
.equ L2VIC_INT_STATUS,      0x380
.equ L2VIC_INT_CLEAR,       0x400
.equ L2VIC_INT_TYPE,        0x280

// Timer interrupt number
.equ TIMER_IRQ_NUM, 3  // From kernel source

// Enable timer interrupt
l2vic_enable_timer_int:
    r3 = ##l2vic_base_addr
    r0 = memw(r3)
    r1 = #TIMER_IRQ_NUM
    r2 = #1
    r2 = asl(r2, r1)  // Create bit mask
    r3 = memw(r0+#L2VIC_INT_ENABLE)
    r3 = or(r3, r2)
    memw(r0+#L2VIC_INT_ENABLE) = r3
    jumpr r31

// Disable timer interrupt
l2vic_disable_timer_int:
    r3 = ##l2vic_base_addr
    r0 = memw(r3)
    r1 = #TIMER_IRQ_NUM
    r2 = #1
    r2 = asl(r2, r1)
    r2 = not(r2)
    r3 = memw(r0+#L2VIC_INT_ENABLE)
    r3 = and(r3, r2)
    memw(r0+#L2VIC_INT_ENABLE) = r3
    jumpr r31

// Clear timer interrupt
l2vic_clear_timer_int:
    r3 = ##l2vic_base_addr
    r0 = memw(r3)
    r1 = #TIMER_IRQ_NUM
    r2 = #1
    r2 = asl(r2, r1)
    memw(r0+#L2VIC_INT_CLEAR) = r2
    jumpr r31
