// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Provides emulation for Linux serial console.
//!
//! This is done by emulating an UART serial port.

use std::collections::VecDeque;
use std::io::{Result, Write};

use vmm_sys_util::eventfd::EventFd;

// Register offsets.
// Receiver and Transmitter registers offset, depending on the I/O
// access type: write -> THR, read -> RBR.
const DATA_OFFSET: u8 = 0;
const IER_OFFSET: u8 = 1;
const IIR_OFFSET: u8 = 2;
const LCR_OFFSET: u8 = 3;
const MCR_OFFSET: u8 = 4;
const LSR_OFFSET: u8 = 5;
const MSR_OFFSET: u8 = 6;
const SCR_OFFSET: u8 = 7;
const DLAB_LOW_OFFSET: u8 = 0;
const DLAB_HIGH_OFFSET: u8 = 1;

const LOOP_SIZE: usize = 0x40;

// Received Data Available interrupt - for letting the driver know that
// there is some pending data to be processed.
const IER_RDA_BIT: u8 = 0b0000_0001;
// Transmitter Holding Register Empty interrupt - for letting the driver
// know that the entire content of the output buffer was sent.
const IER_THR_EMPTY_BIT: u8 = 0b0000_0010;
// The interrupts that are available on 16550 and older models.
const IER_UART_VALID_BITS: u8 = 0b0000_1111;

//FIFO enabled.
const IIR_FIFO_BITS: u8 = 0b1100_0000;
const IIR_NONE_BIT: u8 = 0b0000_0001;
const IIR_THR_EMPTY_BIT: u8 = 0b0000_0010;
const IIR_RDA_BIT: u8 = 0b0000_0100;

const LCR_DLAB_BIT: u8 = 0b1000_0000;

const LSR_DATA_READY_BIT: u8 = 0b0000_0001;
// These two bits help the driver know if the device is ready to accept
// another character.
// THR is empty.
const LSR_EMPTY_THR_BIT: u8 = 0b0010_0000;
// The shift register, which takes a byte from THR and breaks it in bits
// for sending them on the line, is empty.
const LSR_IDLE_BIT: u8 = 0b0100_0000;

// The following five MCR bits allow direct manipulation of the device and
// are available on 16550 and older models.
// Data Terminal Ready.
const MCR_DTR_BIT: u8 = 0b0000_0001;
// Request To Send.
const MCR_RTS_BIT: u8 = 0b0000_0010;
// Auxiliary Output 1.
const MCR_OUT1_BIT: u8 = 0b0000_0100;
// Auxiliary Output 2.
const MCR_OUT2_BIT: u8 = 0b0000_1000;
// Loopback Mode.
const MCR_LOOP_BIT: u8 = 0b0001_0000;

// Clear To Send.
const MSR_CTS_BIT: u8 = 0b0001_0000;
// Data Set Ready.
const MSR_DSR_BIT: u8 = 0b0010_0000;
// Ring Indicator.
const MSR_RI_BIT: u8 = 0b0100_0000;
// Data Carrier Detect.
const MSR_DCD_BIT: u8 = 0b1000_0000;

// The following values can be used to set the baud rate to 9600 bps.
const DEFAULT_BAUD_DIVISOR_HIGH: u8 = 0x00;
const DEFAULT_BAUD_DIVISOR_LOW: u8 = 0x0C;

// No interrupts enabled.
const DEFAULT_INTERRUPT_ENABLE: u8 = 0x00;
// No pending interrupt.
const DEFAULT_INTERRUPT_IDENTIFICATION: u8 = IIR_NONE_BIT;
// We're setting the default to include LSR_EMPTY_THR_BIT and LSR_IDLE_BIT
// and never update those bits because we're working with a virtual device,
// hence we should always be ready to receive more data.
const DEFAULT_LINE_STATUS: u8 = LSR_EMPTY_THR_BIT | LSR_IDLE_BIT;
// 8 bits word length.
const DEFAULT_LINE_CONTROL: u8 = 0b0000_0011;
// Most UARTs need Auxiliary Output 2 set to '1' to enable interrupts.
const DEFAULT_MODEM_CONTROL: u8 = MCR_OUT2_BIT;
const DEFAULT_MODEM_STATUS: u8 = MSR_DSR_BIT | MSR_CTS_BIT | MSR_DCD_BIT;
const DEFAULT_SCRATCH: u8 = 0x00;

/// The serial console emulation is done by emulating a serial COM port.
///
/// Each serial COM port (COM1-4) has an associated Port I/O address base and
/// 12 registers mapped into 8 consecutive Port I/O locations (with the first
/// one being the base).
/// This structure emulates the registers that make sense for UART 16550 (and below)
/// and helps in the interaction between the driver and device by using a fd for
/// notifications. It also writes the guest's output to an `out` Write object.
pub struct Serial<W: Write> {
    // Some UART registers.
    baud_divisor_low: u8,
    baud_divisor_high: u8,
    interrupt_enable: u8,
    interrupt_identification: u8,
    line_control: u8,
    line_status: u8,
    modem_control: u8,
    modem_status: u8,
    scratch: u8,
    // This is the buffer, that is used for achieving the Receiver and Transmitter
    // registers functionality in FIFO mode. Reading from RBR will return the oldest
    // unread byte from the buffer and writing to THR will expand this buffer with
    // one byte.
    in_buffer: VecDeque<u8>,

    // Used for notifying the driver about some in/out events.
    interrupt_evt: EventFd,
    out: W,
}

impl<W: Write> Serial<W> {
    /// Creates a new `Serial` instance which writes the guest's output to
    /// `out` and uses `interrupt_evt` fd to notify the driver about new
    /// events.
    ///
    /// # Arguments
    /// * `interrupt_evt` - The fd that will be used to notify the driver
    ///                     about events.
    /// * `out` - An object for writing guest's output to. In case the output
    ///           is not of interest,
    ///           [std::io::Sink](https://doc.rust-lang.org/std/io/struct.Sink.html)
    ///           can be used here.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::io::sink;
    /// # use vm_superio::Serial;
    /// # use vmm_sys_util::eventfd::EventFd;
    /// let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
    /// let serial = Serial::new(intr_evt.try_clone().unwrap(), Vec::new());
    ///
    /// // std::io::Sink can be used if user is not interested in guest's output.
    /// let serial_with_sink = Serial::new(intr_evt, sink());
    /// ```
    pub fn new(interrupt_evt: EventFd, out: W) -> Serial<W> {
        Serial {
            baud_divisor_low: DEFAULT_BAUD_DIVISOR_LOW,
            baud_divisor_high: DEFAULT_BAUD_DIVISOR_HIGH,
            interrupt_enable: DEFAULT_INTERRUPT_ENABLE,
            interrupt_identification: DEFAULT_INTERRUPT_IDENTIFICATION,
            line_control: DEFAULT_LINE_CONTROL,
            line_status: DEFAULT_LINE_STATUS,
            modem_control: DEFAULT_MODEM_CONTROL,
            modem_status: DEFAULT_MODEM_STATUS,
            scratch: DEFAULT_SCRATCH,
            in_buffer: VecDeque::new(),
            interrupt_evt,
            out,
        }
    }

    /// Provides a reference to the interrupt event fd.
    pub fn interrupt_evt(&self) -> &EventFd {
        &self.interrupt_evt
    }

    fn is_dlab_set(&self) -> bool {
        (self.line_control & LCR_DLAB_BIT) != 0
    }

    fn is_rda_interrupt_enabled(&self) -> bool {
        (self.interrupt_enable & IER_RDA_BIT) != 0
    }

    fn is_thr_interrupt_enabled(&self) -> bool {
        (self.interrupt_enable & IER_THR_EMPTY_BIT) != 0
    }

    fn is_in_loop_mode(&self) -> bool {
        (self.modem_control & MCR_LOOP_BIT) != 0
    }

    fn trigger_interrupt(&mut self) -> Result<()> {
        self.interrupt_evt.write(1)
    }

    fn set_lsr_rda_bit(&mut self) {
        self.line_status |= LSR_DATA_READY_BIT
    }

    fn clear_lsr_rda_bit(&mut self) {
        self.line_status &= !LSR_DATA_READY_BIT
    }

    fn add_interrupt(&mut self, interrupt_bits: u8) {
        self.interrupt_identification &= !IIR_NONE_BIT;
        self.interrupt_identification |= interrupt_bits;
    }

    fn del_interrupt(&mut self, interrupt_bits: u8) {
        self.interrupt_identification &= !interrupt_bits;
        if self.interrupt_identification == 0x00 {
            self.interrupt_identification = IIR_NONE_BIT;
        }
    }

    fn thr_empty_interrupt(&mut self) -> Result<()> {
        if self.is_thr_interrupt_enabled() {
            // Trigger the interrupt only if the identification bit wasn't
            // set or acknowledged.
            if self.interrupt_identification & IIR_THR_EMPTY_BIT == 0 {
                self.add_interrupt(IIR_THR_EMPTY_BIT);
                self.trigger_interrupt()?
            }
        }
        Ok(())
    }

    fn received_data_interrupt(&mut self) -> Result<()> {
        if self.is_rda_interrupt_enabled() {
            // Trigger the interrupt only if the identification bit wasn't
            // set or acknowledged.
            if self.interrupt_identification & IIR_RDA_BIT == 0 {
                self.add_interrupt(IIR_RDA_BIT);
                self.trigger_interrupt()?
            }
        }
        Ok(())
    }

    fn reset_iir(&mut self) {
        self.interrupt_identification = DEFAULT_INTERRUPT_IDENTIFICATION
    }

    /// Handles a write request from the driver at `offset` offset from the
    /// base Port I/O address.
    ///
    /// # Arguments
    /// * `offset` - The offset that will be added to the base PIO address
    ///              for writing to a specific register.
    /// * `value` - The byte that should be written.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use vm_superio::Serial;
    /// # use vmm_sys_util::eventfd::EventFd;
    /// let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
    /// let mut serial = Serial::new(intr_evt, Vec::new());
    ///
    /// // Write 0x01 to THR register.
    /// serial.write(0, 0x01).unwrap();
    /// ```
    pub fn write(&mut self, offset: u8, value: u8) -> Result<()> {
        match offset {
            DLAB_LOW_OFFSET if self.is_dlab_set() => self.baud_divisor_low = value,
            DLAB_HIGH_OFFSET if self.is_dlab_set() => self.baud_divisor_high = value,
            DATA_OFFSET => {
                if self.is_in_loop_mode() {
                    // In loopback mode, what is written in the transmit register
                    // will be immediately found in the receive register, so we
                    // simulate this behavior by adding in `in_buffer` the
                    // transmitted bytes and letting the driver know there is some
                    // pending data to be read, by setting RDA bit and its
                    // corresponding interrupt.
                    if self.in_buffer.len() < LOOP_SIZE {
                        self.in_buffer.push_back(value);
                        self.set_lsr_rda_bit();
                        self.received_data_interrupt()?;
                    }
                } else {
                    self.out.write_all(&[value])?;
                    self.out.flush()?;
                    self.thr_empty_interrupt()?;
                }
            }
            // We want to enable only the interrupts that are available for 16550A (and below).
            IER_OFFSET => self.interrupt_enable = value & IER_UART_VALID_BITS,
            LCR_OFFSET => self.line_control = value,
            MCR_OFFSET => self.modem_control = value,
            SCR_OFFSET => self.scratch = value,
            // We are not interested in writing to other offsets (such as FCR offset).
            _ => {}
        }
        Ok(())
    }

    /// Handles a read request from the driver at `offset` offset from the
    /// base Port I/O address.
    ///
    /// Returns the read value.
    ///
    /// # Arguments
    /// * `offset` - The offset that will be added to the base PIO address
    ///              for reading from a specific register.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use vm_superio::Serial;
    /// # use vmm_sys_util::eventfd::EventFd;
    /// let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
    /// let mut serial = Serial::new(intr_evt, Vec::new());
    ///
    /// // Read from RBR register.
    /// let value = serial.read(0);
    /// ```
    pub fn read(&mut self, offset: u8) -> u8 {
        match offset {
            DLAB_LOW_OFFSET if self.is_dlab_set() => self.baud_divisor_low,
            DLAB_HIGH_OFFSET if self.is_dlab_set() => self.baud_divisor_high,
            DATA_OFFSET => {
                // Here we emulate the reset method for when RDA interrupt
                // was raised (i.e. read the receive buffer and clear the
                // interrupt identification register and RDA bit when no
                // more data is available).
                self.del_interrupt(IIR_RDA_BIT);
                if self.in_buffer.len() <= 1 {
                    self.clear_lsr_rda_bit();
                }
                self.in_buffer.pop_front().unwrap_or_default()
            }
            IER_OFFSET => self.interrupt_enable,
            IIR_OFFSET => {
                // We're enabling FIFO capability by setting the serial port to 16550A:
                // https://elixir.bootlin.com/linux/latest/source/drivers/tty/serial/8250/8250_port.c#L1299.
                let iir = self.interrupt_identification | IIR_FIFO_BITS;
                self.reset_iir();
                iir
            }
            LCR_OFFSET => self.line_control,
            MCR_OFFSET => self.modem_control,
            LSR_OFFSET => self.line_status,
            MSR_OFFSET => {
                if self.is_in_loop_mode() {
                    // In loopback mode, the four modem control inputs (CTS, DSR, RI, DCD) are
                    // internally connected to the four modem control outputs (RTS, DTR, OUT1, OUT2).
                    // This way CTS is controlled by RTS, DSR by DTR, RI by OUT1 and DCD by OUT2.
                    // (so they will basically contain the same value).
                    let mut msr =
                        self.modem_status & !(MSR_DSR_BIT | MSR_CTS_BIT | MSR_RI_BIT | MSR_DCD_BIT);
                    if (self.modem_control & MCR_DTR_BIT) != 0 {
                        msr |= MSR_DSR_BIT;
                    }
                    if (self.modem_control & MCR_RTS_BIT) != 0 {
                        msr |= MSR_CTS_BIT;
                    }
                    if (self.modem_control & MCR_OUT1_BIT) != 0 {
                        msr |= MSR_RI_BIT;
                    }
                    if (self.modem_control & MCR_OUT2_BIT) != 0 {
                        msr |= MSR_DCD_BIT;
                    }
                    msr
                } else {
                    self.modem_status
                }
            }
            SCR_OFFSET => self.scratch,
            _ => 0,
        }
    }

    /// Helps in sending more bytes to the guest in one shot, by storing
    /// `input` bytes in UART buffer and letting the driver know there is
    /// some pending data to be read by setting RDA bit and its corresponding
    /// interrupt when not already triggered.
    ///
    /// # Arguments
    /// * `input` - The data to be sent to the guest.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use vm_superio::Serial;
    /// # use vmm_sys_util::eventfd::EventFd;
    /// let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
    /// let mut serial = Serial::new(intr_evt, Vec::new());
    ///
    /// serial.enqueue_raw_bytes(&[b'a', b'b', b'c']).unwrap();
    /// ```
    pub fn enqueue_raw_bytes(&mut self, input: &[u8]) -> Result<()> {
        if !self.is_in_loop_mode() {
            self.in_buffer.extend(input);
            self.set_lsr_rda_bit();
            self.received_data_interrupt()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::sink;

    const RAW_INPUT_BUF: [u8; 3] = [b'a', b'b', b'c'];

    #[test]
    fn test_serial_output() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, Vec::new());

        // Valid one char at a time writes.
        RAW_INPUT_BUF
            .iter()
            .for_each(|&c| serial.write(DATA_OFFSET, c).unwrap());
        assert_eq!(serial.out.as_slice(), &RAW_INPUT_BUF);
    }

    #[test]
    fn test_serial_raw_input() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt.try_clone().unwrap(), sink());

        serial.write(IER_OFFSET, IER_RDA_BIT).unwrap();
        serial.enqueue_raw_bytes(&RAW_INPUT_BUF).unwrap();

        // Verify the serial raised an interrupt.
        assert_eq!(intr_evt.read().unwrap(), 1);

        // `DATA_READY` bit should've been set by `enqueue_raw_bytes()`.
        let mut lsr = serial.read(LSR_OFFSET);
        assert_ne!(lsr & LSR_DATA_READY_BIT, 0);

        // Verify reading the previously pushed buffer.
        RAW_INPUT_BUF.iter().for_each(|&c| {
            lsr = serial.read(LSR_OFFSET);
            // `DATA_READY` bit won't be cleared until there is
            // just one byte left in the receive buffer.
            assert_ne!(lsr & LSR_DATA_READY_BIT, 0);
            assert_eq!(serial.read(DATA_OFFSET), c);
            // The Received Data Available interrupt bit should be
            // cleared after reading the first pending byte.
            assert_eq!(
                serial.interrupt_identification,
                DEFAULT_INTERRUPT_IDENTIFICATION
            );
        });

        lsr = serial.read(LSR_OFFSET);
        assert_eq!(lsr & LSR_DATA_READY_BIT, 0);
    }

    #[test]
    fn test_serial_thr() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt.try_clone().unwrap(), sink());

        serial.write(IER_OFFSET, IER_THR_EMPTY_BIT).unwrap();
        assert_eq!(
            serial.interrupt_enable,
            IER_THR_EMPTY_BIT & IER_UART_VALID_BITS
        );
        serial.write(DATA_OFFSET, b'a').unwrap();

        // Verify the serial raised an interrupt.
        assert_eq!(intr_evt.read().unwrap(), 1);

        let ier = serial.read(IER_OFFSET);
        assert_eq!(ier & IER_UART_VALID_BITS, IER_THR_EMPTY_BIT);
        let iir = serial.read(IIR_OFFSET);
        // Verify the raised interrupt is indeed the empty THR one.
        assert_ne!(iir & IIR_THR_EMPTY_BIT, 0);

        // When reading from IIR offset, the returned value will tell us that
        // FIFO feature is enabled.
        assert_eq!(iir, IIR_THR_EMPTY_BIT | IIR_FIFO_BITS);
        assert_eq!(
            serial.interrupt_identification,
            DEFAULT_INTERRUPT_IDENTIFICATION
        );
    }

    #[test]
    fn test_serial_loop_mode() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt.try_clone().unwrap(), sink());

        serial.write(MCR_OFFSET, MCR_LOOP_BIT).unwrap();
        serial.write(IER_OFFSET, IER_RDA_BIT).unwrap();

        for value in 0..LOOP_SIZE as u8 {
            serial.write(DATA_OFFSET, value).unwrap();
            assert_eq!(intr_evt.read().unwrap(), 1);
            assert_eq!(serial.in_buffer.len(), 1);
            // Immediately read a pushed value.
            assert_eq!(serial.read(DATA_OFFSET), value);
        }

        assert_eq!(serial.line_status & LSR_DATA_READY_BIT, 0);

        for value in 0..LOOP_SIZE as u8 {
            serial.write(DATA_OFFSET, value).unwrap();
        }

        assert_eq!(intr_evt.read().unwrap(), 1);
        assert_eq!(serial.in_buffer.len(), LOOP_SIZE);

        // Read the pushed values at the end.
        for value in 0..LOOP_SIZE as u8 {
            assert_ne!(serial.line_status & LSR_DATA_READY_BIT, 0);
            assert_eq!(serial.read(DATA_OFFSET), value);
        }
        assert_eq!(serial.line_status & LSR_DATA_READY_BIT, 0);
    }

    #[test]
    fn test_serial_dlab() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        // For writing to DLAB registers, `DLAB` bit from LCR should be set.
        serial.write(LCR_OFFSET, LCR_DLAB_BIT).unwrap();
        serial.write(DLAB_HIGH_OFFSET, 0x12).unwrap();
        assert_eq!(serial.read(DLAB_LOW_OFFSET), DEFAULT_BAUD_DIVISOR_LOW);
        assert_eq!(serial.read(DLAB_HIGH_OFFSET), 0x12);

        serial.write(DLAB_LOW_OFFSET, 0x34).unwrap();

        assert_eq!(serial.read(DLAB_LOW_OFFSET), 0x34);
        assert_eq!(serial.read(DLAB_HIGH_OFFSET), 0x12);

        // If LCR_DLAB_BIT is not set, the values from `DLAB_LOW_OFFSET` and
        // `DLAB_HIGH_OFFSET` won't be the expected ones.
        serial.write(LCR_OFFSET, 0x00).unwrap();
        assert_ne!(serial.read(DLAB_LOW_OFFSET), 0x12);
        assert_ne!(serial.read(DLAB_HIGH_OFFSET), 0x34);
    }

    #[test]
    fn test_basic_register_accesses() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        // Writing to these registers does not alter the initial values to be written
        // and reading from these registers just returns those values, without
        // modifying them.
        let basic_register_accesses = [LCR_OFFSET, MCR_OFFSET, SCR_OFFSET];
        for offset in basic_register_accesses.iter() {
            serial.write(*offset, 0x12).unwrap();
            assert_eq!(serial.read(*offset), 0x12);
        }
    }

    #[test]
    fn test_invalid_access() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        // Check if reading from an offset outside 0-7 returns for sure 0.
        serial.write(SCR_OFFSET + 1, 5).unwrap();
        assert_eq!(serial.read(SCR_OFFSET + 1), 0);
    }

    #[test]
    fn test_serial_msr() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        assert_eq!(serial.read(MSR_OFFSET), DEFAULT_MODEM_STATUS);

        // Activate loopback mode.
        serial.write(MCR_OFFSET, MCR_LOOP_BIT).unwrap();

        // In loopback mode, MSR won't contain the default value anymore.
        assert_ne!(serial.read(MSR_OFFSET), DEFAULT_MODEM_STATUS);
        assert_eq!(serial.read(MSR_OFFSET), 0x00);

        // Depending on which bytes we enable for MCR, MSR will be modified accordingly.
        serial
            .write(MCR_OFFSET, DEFAULT_MODEM_CONTROL | MCR_LOOP_BIT)
            .unwrap();
        // DEFAULT_MODEM_CONTROL sets OUT2 from MCR to 1. In loopback mode, OUT2 is equivalent
        // to DCD bit from MSR.
        assert_eq!(serial.read(MSR_OFFSET), MSR_DCD_BIT);

        // The same should happen with OUT1 and RI.
        serial
            .write(MCR_OFFSET, MCR_OUT1_BIT | MCR_LOOP_BIT)
            .unwrap();
        assert_eq!(serial.read(MSR_OFFSET), MSR_RI_BIT);

        serial
            .write(MCR_OFFSET, MCR_LOOP_BIT | MCR_DTR_BIT | MCR_RTS_BIT)
            .unwrap();
        // DSR and CTS from MSR are "matching wires" to DTR and RTS from MCR (so they will
        // have the same value).
        assert_eq!(serial.read(MSR_OFFSET), MSR_DSR_BIT | MSR_CTS_BIT);
    }
}
