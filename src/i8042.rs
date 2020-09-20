// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Provides emulation for a super minimal i8042 controller.
//!
//! This emulates just the CPU reset command.

use std::fmt::{self, Display, Formatter};
use std::{io, result};
use vmm_sys_util::eventfd::EventFd;

// Offset of the command register, for write accesses (port 0x64). The same
// offset can be used, in case of read operations, to access the status
// register (in which we are not interested for an i8042 that only knows
// about reset).
const COMMAND_OFFSET: u8 = 4;

// Reset CPU command.
const CMD_RESET_CPU: u8 = 0xFE;

/// Errors encountered while handling i8042 operations.
#[derive(Debug)]
pub enum Error {
    /// Failed to trigger interrupt.
    TriggerInterrupt(io::Error),
}

/// Specialized Result type for [i8042 Errors](enum.Error.html).
pub type Result<T> = result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Error::TriggerInterrupt(_) => "Cannot trigger interrupt",
            }
        )
    }
}

/// An i8042 PS/2 controller that emulates just enough to shutdown the machine.
pub struct I8042Device {
    /// CPU reset event fd. We will trigger this event when the guest issues
    /// the reset CPU command.
    reset_evt: EventFd,
}

impl I8042Device {
    /// Constructs an i8042 device that will signal the given event when the
    /// guest requests it.
    pub fn new(reset_evt: EventFd) -> I8042Device {
        I8042Device { reset_evt }
    }
}

impl I8042Device {
    /// Handles a read request from the driver at `_offset` offset from the
    /// base I/O address.
    ///
    /// Returns the read value, which at this moment is 0x00, since we're not
    /// interested in an i8042 operation other than CPU reset.
    ///
    /// # Arguments
    /// * `_offset` - The offset that will be added to the base address
    ///              for writing to a specific register.
    pub fn read(&mut self, _offset: u8) -> u8 {
        0x00
    }

    /// Handles a write request from the driver at `offset` offset from the
    /// base I/O address.
    ///
    /// # Arguments
    /// * `offset` - The offset that will be added to the base address
    ///              for writing to a specific register.
    /// * `value` - The byte that should be written.
    pub fn write(&mut self, offset: u8, value: u8) -> Result<()> {
        match offset {
            COMMAND_OFFSET if value == CMD_RESET_CPU => {
                // Trigger the exit event fd.
                self.reset_evt.write(1).map_err(Error::TriggerInterrupt)
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i8042_read_write_and_event() {
        let reset_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut i8042 = I8042Device::new(reset_evt.try_clone().unwrap());

        assert_eq!(i8042.read(0), 0);

        // Check if reset works.
        i8042.write(COMMAND_OFFSET, CMD_RESET_CPU).unwrap();
        assert_eq!(reset_evt.read().unwrap(), 1);

        // Write something different than CPU reset and check that the reset event
        // was not triggered. For this we have to write 1 to the reset event fd, so
        // that read doesn't block.
        assert!(reset_evt.write(1).is_ok());
        i8042.write(COMMAND_OFFSET, CMD_RESET_CPU + 1).unwrap();
        assert_eq!(reset_evt.read().unwrap(), 1);
    }
}
