# vm-superio


The `vm-superio` crate provides emulation for legacy devices. For now, it offers
this support only for the
[Linux serial console](https://en.wikipedia.org/wiki/Linux_console).

## Serial Console

### Design

The console emulation is done by emulating a hybrid between
[UART 8250 serial port](https://en.wikibooks.org/w/index.php?title=Serial_Programming/8250_UART_Programming&section=15#Serial_COM_Port_Memory_and_I/O_Allocation)
and [UART 16550 serial port](https://en.wikipedia.org/wiki/16550_UART).
For a VMM to be able to use this device, besides the emulation part which is
covered in this crate, the serial port should be added to the microVM’s PIO bus,
the serial backend should be defined and if and how the event handling is done.

The following UART registers are emulated via the `Serial` struct: DLL, IER,
DLH, IIR, LCR, LSR, MCR, MSR and SR (more details about these,
[here](http://%20https//en.wikibooks.org/w/index.php?title=Serial_Programming/8250_UART_Programming%C2%A7ion=15#UART_Registers)).
The Fifo Control Register (FCR) is not emulated since we are not interested in
directly controlling the FIFOs and The Receiver Buffer and Transmitter Holding
Buffer registers (THR and RBR) functionality is simplified, yet extended by
using a single buffer. This buffer helps in testing the UART when running in
loopback mode and for keeping the guest output to a `Write` object (`out`).
The VMM that will use the serial console, when instantiating a `Serial`, will
have to provide a `Write` object for it (for example `io::Stdout` or
`io::Sink`).
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
