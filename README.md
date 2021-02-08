# vm-superio


The `vm-superio` crate provides emulation for legacy devices. For now, it offers
this support only for the
[Linux serial console](https://en.wikipedia.org/wiki/Linux_console) and a minimal
[i8042 PS/2 Controller](https://wiki.osdev.org/%228042%22_PS/2_Controller).

## Serial Console

### Design

The console emulation is done by emulating a simple
[UART 16550A serial port](https://en.wikipedia.org/wiki/16550_UART) with a
64-byte FIFO.
This UART is an improvement of the original
[UART 8250 serial port](https://en.wikibooks.org/w/index.php?title=Serial_Programming/8250_UART_Programming&section=15#Serial_COM_Port_Memory_and_I/O_Allocation),
mostly because of the FIFO buffers that allow storing more than one byte at a
time, which, in virtualized environments, is essential.

For a VMM to be able to use this device, besides the emulation part which is
covered in this crate, the VMM needs to do the following operations:
- add the serial port to the Bus (either PIO or MMIO)
- define the serial backend
- event handling (optional)

The following UART registers are emulated via the
[`Serial` structure](src/serial.rs): DLL, IER, DLH, IIR, LCR, LSR, MCR, MSR and
SR (a brief, but nice presentation about these,
[here](https://www.lammertbies.nl/comm/info/serial-uart#regs)).
The Fifo Control Register (FCR) is not emulated; there is no support yet for
directly controlling the FIFOs (which, in this implementation, are always
enabled). The serial console implements only the RX FIFO (and its
corresponding RBR register). The RX buffer helps in testing the UART when
running in loopback mode and for sending more bytes to the guest in one shot.
The TX FIFO is trivially implemented by immediately writing a byte coming from
the driver to an `io::Write` object (`out`), which can be, for example,
`io::Stdout` or `io::Sink`. This object has to be provided when
[initializing the serial console](https://docs.rs/vm-superio/0.1.1/vm_superio/serial/struct.Serial.html#method.new).
A `Trigger` object is the currently used mechanism for notifying the driver
about in/out events that need to be handled.

### Usage

The interaction between the serial console and its driver, at the emulation
level, is done by the two `read` and `write` specific methods, which handle
one byte accesses. For sending more input, `enqueue_raw_bytes` can be used.

## i8042 PS/2 Controller

The i8042 PS/2 controller emulates, at this point, only the
[CPU reset command](https://wiki.osdev.org/%228042%22_PS/2_Controller#CPU_Reset)
which is needed for announcing the VMM about the guest's shutdown.

## Threat model

For the serial console, there is no monitoring of the amount of data that is
written to the `out` object.

*Threat*: A malicious guest can fill up the host disk by generating a high
amount of data to be written to the serial output.  
*Mitigation*: There is no mitigation implemented at the serial console emulation
level. To mitigate this at the VMM level, it is recommended to use as output a
resource that has a fixed size (e.g. ring buffer or a named pipe).

## License

This project is licensed under either of

- [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0
- [BSD-3-Clause License](https://opensource.org/licenses/BSD-3-Clause)
