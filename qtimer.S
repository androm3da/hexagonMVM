.section .text.qtimer_interface

// QTimer register offsets (based on QEMU model)
.equ QTIMER_CTRL,       0x00
.equ QTIMER_LOAD,       0x04
.equ QTIMER_VALUE,      0x08
.equ QTIMER_RELOAD,     0x0C
.equ QTIMER_INTSTATUS,  0x10
.equ QTIMER_INTCLEAR,   0x14

// Get timer frequency
// Input: None
// Output: r0 = frequency in Hz
qtimer_get_freq:
    // Return fixed frequency for now (19.2 MHz typical)
    r0 = ##19200000
    jumpr r31

// Get timer resolution
// Output: r0 = resolution in nanoseconds
qtimer_get_res:
    // Calculate based on frequency
    r0 = ##52  // ~52ns for 19.2MHz
    jumpr r31

// Get current timer value
// Output: r0:r1 = 64-bit timer value
qtimer_get_time:
    r2 = ##qtimer_base_addr
    r1 = memw(r2)
    r0 = memw(r1+#QTIMER_VALUE)
    r1 = #0  // Upper 32 bits (if 64-bit timer)
    jumpr r31

// Get timeout value
// Output: r0 = timeout value
qtimer_get_timeout:
    r2 = ##qtimer_base_addr
    r1 = memw(r2)
    r0 = memw(r1+#QTIMER_LOAD)
    jumpr r31

// Set timeout value
// Input: r0 = timeout value
qtimer_set_timeout:
    r2 = ##qtimer_base_addr
    r1 = memw(r2)
    memw(r1+#QTIMER_LOAD) = r0
    memw(r1+#QTIMER_RELOAD) = r0
    jumpr r31

// Set delta timeout
// Input: r0 = delta value to add
qtimer_delta_timeout:
    r2 = ##qtimer_base_addr
    r1 = memw(r2)
    r3 = memw(r1+#QTIMER_VALUE)
    r0 = add(r0, r3)
    memw(r1+#QTIMER_LOAD) = r0
    jumpr r31
