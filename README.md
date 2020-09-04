# vm-superio


The `vm-superio` crate provides emulation for legacy devices. For now, it offers
this support only for the
[Linux serial console](https://en.wikipedia.org/wiki/Linux_console).

## Serial Console

### Design

The console emulation is done by emulating a simple
[UART 16550A serial port](https://en.wikipedia.org/wiki/16550_UART).
This UART is an improvement of the original
[UART 8250 serial port](https://en.wikibooks.org/w/index.php?title=Serial_Programming/8250_UART_Programming&section=15#Serial_COM_Port_Memory_and_I/O_Allocation),
mostly because of the FIFO buffers that allow storing more than one byte at a
time, which, in virtualized environments, is essential.
For a VMM to be able to use this device, besides the emulation part which is
covered in this crate, the serial port should be added to the microVM’s PIO bus,
the serial backend should be defined and if and how the event handling is done.

The following UART registers are emulated via the
[`Serial` structure](src/serial.rs): DLL, IER, DLH, IIR, LCR, LSR, MCR, MSR and
SR (a brief, but nice presentation about these,
[here](https://www.lammertbies.nl/comm/info/serial-uart#regs)).
The Fifo Control Register (FCR) is not emulated since we are not interested in
directly controlling the FIFOs (which, in our case, are always enabled) and
The Receiver Buffer and Transmitter Holding Buffer registers (THR and RBR)
functionality is simplified, yet extended by using a single FIFO buffer. When
reading from RBR, the oldest unread byte from the FIFO is returned and when
writing to THR, the FIFO buffer is extended with the new byte. This buffer helps
in testing the UART when running in loopback mode and for keeping the guest
output to a `Write` object (`out`). The VMM that will use the serial console,
when instantiating a `Serial`, will have to provide a `Write` object for it (for
example `io::Stdout` or `io::Sink`).
The `interrupt_evt` fd is the currently used mechanism for notifying the driver
when changes in the previously mentioned buffer happened that need to be
handled, but further abstractions may come here; see
[tracking issue](https://github.com/rust-vmm/vm-superio/issues/7).

### Usage

The interaction between the serial console and its driver, at the emulation
level, is done by the two `read` and `write` specific methods, which handle
one byte accesses. For sending more input, `enqueue_raw_bytes` can be used. 

## License

This project is licensed under either of

- [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0
- [BSD-3-Clause License](https://opensource.org/licenses/BSD-3-Clause)
