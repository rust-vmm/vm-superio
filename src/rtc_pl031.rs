// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Provides emulation for a minimal ARM PL031 Real Time Clock.
//!
//! This module implements a PL031 Real Time Clock (RTC) that provides a long
//! time base counter. This is achieved by generating an interrupt signal after
//! counting for a programmed number of cycles of a real-time clock input.
//!
use std::time::Instant;

// As you can see in
//  https://static.docs.arm.com/ddi0224/c/real_time_clock_pl031_r1p3_technical_reference_manual_DDI0224C.pdf
//  at section 3.2 Summary of RTC registers, the total size occupied by this
//  device is 0x000 -> 0xFFC + 4 = 0x1000 bytes.

// From 0x0 to 0x1C we have following registers:
const RTCDR: u16 = 0x000; // Data Register.
const RTCMR: u16 = 0x004; // Match Register.
const RTCLR: u16 = 0x008; // Load Register.
const RTCCR: u16 = 0x00C; // Control Register.
const RTCIMSC: u16 = 0x010; // Interrupt Mask Set or Clear Register.
const RTCRIS: u16 = 0x014; // Raw Interrupt Status.
const RTCMIS: u16 = 0x018; // Masked Interrupt Status.
const RTCICR: u16 = 0x01C; // Interrupt Clear Register.

// From 0x020 to 0xFDC => reserved space.

// From 0xFE0 to 0xFFF => Peripheral and PrimeCell Identification Registers
//  These are read-only registers, so we store their values in a constant array.
//  The values are found in the 'Reset value' column of Table 3.1 (Summary of
//  RTC registers) in the the reference manual linked above.
const AMBA_IDS: [u8; 8] = [0x31, 0x10, 0x04, 0x00, 0x0d, 0xf0, 0x05, 0xb1];

// Since we are specifying the AMBA IDs in an array, instead of in individual
// registers, these constants bound the register addresses where these IDs
// would normally be located.
const AMBA_ID_LOW: u16 = 0xFE0;
const AMBA_ID_HIGH: u16 = 0xFFF;

/// A PL031 Real Time Clock (RTC) that emulates a long time base counter.
///
/// This structure emulates the registers for the RTC.
///
/// # Example
///
/// ```rust
/// # use std::thread;
/// # use std::io::Error;
/// # use std::ops::Deref;
/// # use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
/// # use vm_superio::RTC;
///
/// let mut data = [0; 4];
/// let mut rtc = RTC::new();
/// const RTCDR: u16 = 0x0; // Data Register.
/// const RTCLR: u16 = 0x8; // Load Register.
///
/// // Write system time since UNIX_EPOCH in seconds to the load register.
/// let v = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
/// data = (v as u32).to_le_bytes();
/// rtc.write(RTCLR, &data);
///
/// // Read the value back out of the load register.
/// rtc.read(RTCLR, &mut data);
/// assert_eq!((v as u32), u32::from_le_bytes(data));
///
/// // Sleep for 1.5 seconds to let the counter tick.
/// let delay = Duration::from_millis(1500);
/// thread::sleep(delay);
///
/// // Read the current RTC value from the Data Register
/// rtc.read(RTCDR, &mut data);
/// assert!(u32::from_le_bytes(data) > (v as u32));
/// ```
pub struct RTC {
    // Counts up from 1 on reset at 1Hz (emulated).
    counter: Instant,

    // The offset value applied to the counter to get the RTC value.
    lr: u32,

    // The MR register is used for implementing the RTC alarm. A
    // real time clock alarm is a feature that can be used to allow
    // a computer to 'wake up' after shut down to execute tasks
    // every day or on a certain day. It can sometimes be found in
    // the 'Power Management' section of a motherboard's BIOS setup.
    // This is not currently implemented, so we raise an error.
    // TODO: Implement the match register functionality.
    mr: u32,

    // The interrupt mask.
    imsc: u32,

    // The raw interrupt value.
    ris: u32,
}

impl RTC {
    /// Creates a new `AMBA PL031 RTC` instance.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `RTC`](struct.RTC.html#example).
    pub fn new() -> RTC {
        RTC {
            // Counts up from 1 on reset at 1Hz (emulated).
            counter: Instant::now(),

            // The load register is initialized to zero.
            lr: 0,

            // The match register is initialised to zero (not currently used).
            // TODO: Implement the match register functionality.
            mr: 0,

            // The interrupt mask is initialised as not set.
            imsc: 0,

            // The raw interrupt is initialised as not asserted.
            ris: 0,
        }
    }

    fn get_rtc_value(&self) -> u32 {
        // Add the counter offset to the seconds elapsed since reset.
        // Using wrapping_add() eliminates the possibility of a panic
        // and makes the desired behaviour (a wrap) explicit.
        (self.counter.elapsed().as_secs() as u32).wrapping_add(self.lr)
    }

    /// Handles a write request from the driver at `offset` offset from the
    /// base register address.
    ///
    /// # Arguments
    /// * `offset` - The offset from the base register specifying
    ///              the register to be written.
    /// * `data` - The little endian, 4 byte array to write to the register
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `RTC`](struct.RTC.html#example).
    pub fn write(&mut self, offset: u16, data: &[u8; 4]) {
        let val = u32::from_le_bytes(*data);

        match offset {
            RTCMR => {
                // Set the match register, though this is not currently used.
                // TODO: Implement the match register functionality.
                self.mr = val;
            }
            RTCLR => {
                // Writing to the load register adjusts both the load register
                // and the counter to ensure that a write to RTCLR followed by
                // an immediate read of RTCDR will return the loaded value.
                self.counter = Instant::now();
                self.lr = val;
            }
            RTCCR => {
                // Writing 1 to the control register resets the RTC value,
                // which means both the counter and load register are reset.
                if val == 1 {
                    self.counter = Instant::now();
                    self.lr = 0;
                }
            }
            RTCIMSC => {
                // Set or clear the interrupt mask.
                self.imsc = val & 1;
            }
            RTCICR => {
                // Writing 1 clears the interrupt.
                self.ris &= !val;
            }
            _ => {
                // Writes to RTCDR, RTCRIS, RTCMIS, or an invalid offset
                // are ignored.
            }
        };
    }

    /// Handles a read request from the driver at `offset` offset from the
    /// base register address.
    ///
    /// # Arguments
    /// * `offset` - The offset from the base register specifying
    ///              the register to be read.
    /// * `data` - The little-endian, 4 byte array storing the read value.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `RTC`](struct.RTC.html#example).
    pub fn read(&mut self, offset: u16, data: &mut [u8; 4]) {
        let v = if (AMBA_ID_LOW..=AMBA_ID_HIGH).contains(&offset) {
            let index = ((offset - AMBA_ID_LOW) >> 2) as usize;
            u32::from(AMBA_IDS[index])
        } else {
            match offset {
                RTCDR => self.get_rtc_value(),
                RTCMR => {
                    // Read the match register, though this is not currently used.
                    // TODO: Implement the match register functionality.
                    self.mr
                }
                RTCLR => self.lr,
                RTCCR => 1, // RTC is always enabled.
                RTCIMSC => self.imsc,
                RTCRIS => self.ris,
                RTCMIS => self.ris & self.imsc,
                _ => {
                    // If the offset is invalid, do nothing.
                    return;
                }
            }
        };

        *data = v.to_le_bytes();
    }
}

impl Default for RTC {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    // TODO: Implement metrics with the rust-vmm crate
    // use vmm_sys_util::metric::Metric;

    #[test]
    fn test_data_register() {
        // Verify we can read the Data Register, but not write to it,
        // and that the Data Register RTC count increments over time.
        // Also, test the Default constructor for RTC.
        let mut rtc: RTC = Default::default();
        let mut data = [0; 4];

        // Read the data register.
        rtc.read(RTCDR, &mut data);
        let first_read = u32::from_le_bytes(data);

        // Sleep for 1.5 seconds to let the counter tick.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let second_read = u32::from_le_bytes(data);

        // The second time should be greater than the first
        assert!(second_read > first_read);

        // Sleep for 1.5 seconds to let the counter tick.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Writing the data register should have no effect.
        data = 0u32.to_le_bytes();
        rtc.write(RTCDR, &data);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let third_read = u32::from_le_bytes(data);

        // The third time should be greater than the second.
        assert!(third_read > second_read);
    }

    #[test]
    fn test_match_register() {
        // Test reading and writing to the match register.
        // TODO: Implement the alarm functionality and confirm an interrupt
        // is raised when the match register is set.
        let mut rtc = RTC::new();
        let mut data: [u8; 4];

        // Write to and read the value back out of the match register.
        data = 123u32.to_le_bytes();
        rtc.write(RTCMR, &data);
        rtc.read(RTCMR, &mut data);
        assert_eq!(123, u32::from_le_bytes(data));
    }

    #[test]
    fn test_load_register() {
        // Read and write to the load register to confirm we can both
        // set the RTC value forward and backward.
        let mut rtc = RTC::new();
        let mut data = [0; 4];

        // Get the RTC value with a load register of 0 (the initial value).
        rtc.read(RTCDR, &mut data);
        let old_val = u32::from_le_bytes(data);

        // Write system time since UNIX_EPOCH in seconds to the load register.
        let lr = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        data = (lr as u32).to_le_bytes();
        rtc.write(RTCLR, &data);

        // Read the load register and verify it matches the value just loaded.
        rtc.read(RTCLR, &mut data);
        assert_eq!((lr as u32), u32::from_le_bytes(data));

        // Read the data register and verify it matches the value just loaded.
        // Note that this assumes less than 1 second has elapsed between
        // setting RTCLR and this read (based on the RTC counter
        // tick rate being 1Hz).
        rtc.read(RTCDR, &mut data);
        assert_eq!((lr as u32), u32::from_le_bytes(data));

        // Confirm that the new RTC value is greater than the old
        let new_val = u32::from_le_bytes(data);
        assert!(new_val > old_val);

        // Sleep for 1.5 seconds to let the counter tick.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Reset the RTC value to 0 and confirm it was reset.
        let lr = 0;
        data = (lr as u32).to_le_bytes();
        rtc.write(RTCLR, &data);

        // Read the data register and verify it has been reset.
        rtc.read(RTCDR, &mut data);
        assert_eq!((lr as u32), u32::from_le_bytes(data));
    }

    #[test]
    fn test_rtc_value_overflow() {
        // Verify that the RTC value will wrap on overflow instead of panic.
        let mut rtc = RTC::new();
        let mut data: [u8; 4];

        // Write u32::MAX to the load register
        let lr_max = u32::MAX;
        data = lr_max.to_le_bytes();
        rtc.write(RTCLR, &data);

        // Read the load register and verify it matches the value just loaded.
        rtc.read(RTCLR, &mut data);
        assert_eq!(lr_max, u32::from_le_bytes(data));

        // Read the data register and verify it matches the value just loaded.
        // Note that this assumes less than 1 second has elapsed between
        // setting RTCLR and this read (based on the RTC counter
        // tick rate being 1Hz).
        rtc.read(RTCDR, &mut data);
        assert_eq!(lr_max, u32::from_le_bytes(data));

        // Sleep for 1.5 seconds to let the counter tick. This should
        // cause the RTC value to overflow and wrap.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Read the data register and verify it has wrapped around.
        rtc.read(RTCDR, &mut data);
        assert!(lr_max > u32::from_le_bytes(data));
    }

    #[test]
    fn test_interrupt_mask_set_clear_register() {
        // Test setting and clearing the interrupt mask bit.
        let mut rtc = RTC::new();
        let mut data: [u8; 4];

        // Manually set the raw interrupt.
        rtc.ris = 1;

        // Set the mask bit.
        data = 1u32.to_le_bytes();
        rtc.write(RTCIMSC, &data);

        // Confirm the mask bit is set.
        rtc.read(RTCIMSC, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));

        // Confirm the raw and masked interrupts are set.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));

        // Clear the mask bit.
        data = 0u32.to_le_bytes();
        rtc.write(RTCIMSC, &data);

        // Confirm the mask bit is cleared.
        rtc.read(RTCIMSC, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));

        // Confirm the raw interrupt is set and the masked
        // interrupt is not.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));
    }

    #[test]
    fn test_interrupt_clear_register() {
        // Test clearing the interrupt.
        let mut rtc = RTC::new();
        let mut data = [0; 4];

        // Manually set the raw interrupt and interrupt mask.
        rtc.ris = 1;
        rtc.imsc = 1;

        // Confirm the raw and masked interrupts are set.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));

        // Write to the interrupt clear register.
        data = 1u32.to_le_bytes();
        rtc.write(RTCICR, &data);

        // Confirm the raw and masked interrupts are cleared.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));

        // Confirm reading from RTCICR has no effect.
        data = 123u32.to_le_bytes();
        rtc.read(RTCICR, &mut data);
        let v = u32::from_le_bytes(data);
        assert_eq!(v, 123);
    }

    #[test]
    fn test_control_register() {
        // Writing 1 to the Control Register should reset the RTC value.
        // Writing 0 should have no effect.
        let mut rtc = RTC::new();
        let mut data: [u8; 4];

        // Write system time since UNIX_EPOCH in seconds to the load register.
        let lr = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        data = (lr as u32).to_le_bytes();
        rtc.write(RTCLR, &data);

        // Get the RTC value.
        rtc.read(RTCDR, &mut data);
        let old_val = u32::from_le_bytes(data);

        // Reset the RTC value by writing 1 to RTCCR.
        data = 1u32.to_le_bytes();
        rtc.write(RTCCR, &data);

        // Get the RTC value.
        rtc.read(RTCDR, &mut data);
        let new_val = u32::from_le_bytes(data);

        // The new value should be less than the old value.
        assert!(new_val < old_val);

        // Attempt to clear the control register should have no effect on
        // either the RTCCR value or the RTC value.
        data = 0u32.to_le_bytes();
        rtc.write(RTCCR, &data);

        // Read the RTCCR value and confirm it's still 1.
        rtc.read(RTCCR, &mut data);
        let v = u32::from_le_bytes(data);
        assert_eq!(v, 1);

        // Sleep for 1.5 seconds to let the counter tick.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Read the RTC value and confirm it has incremented.
        let old_val = new_val;
        rtc.read(RTCDR, &mut data);
        let new_val = u32::from_le_bytes(data);
        assert!(new_val > old_val);
    }

    #[test]
    fn test_raw_interrupt_status_register() {
        // Writing to the Raw Interrupt Status Register should have no effect,
        // and reading should return the value of RTCRIS.
        let mut rtc = RTC::new();
        let mut data = [0; 4];

        // Set the raw interrupt for testing.
        rtc.ris = 1u32;

        // Read the current value of RTCRIS.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);

        // Attempt to write to RTCRIS.
        data = 0u32.to_le_bytes();
        rtc.write(RTCRIS, &data);

        // Read the current value of RTCRIS and confirm it's unchanged.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);
    }

    #[test]
    fn test_mask_interrupt_status_register() {
        // Writing to the Masked Interrupt Status Register should have no effect,
        // and reading should return the value of RTCRIS & RTCIMSC.
        let mut rtc = RTC::new();
        let mut data = [0; 4];

        // Set the raw interrupt for testing.
        rtc.ris = 1u32;

        // Confirm the mask bit is not set.
        rtc.read(RTCIMSC, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));

        // Read the current value of RTCMIS. Since the interrupt mask is
        // initially 0, the interrupt should not be masked and reading RTCMIS
        // should return 0.
        rtc.read(RTCMIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 0);

        // Set the mask bit.
        data = 1u32.to_le_bytes();
        rtc.write(RTCIMSC, &data);

        // Read the current value of RTCMIS. Since the interrupt mask is
        // now set, the masked interrupt should be set.
        rtc.read(RTCMIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);

        // Attempt to write to RTCMIS should have no effect.
        data = 0u32.to_le_bytes();
        rtc.write(RTCMIS, &data);

        // Read the current value of RTCMIS and confirm it's unchanged.
        rtc.read(RTCMIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);
    }

    #[test]
    fn test_read_only_register_addresses() {
        let mut rtc = RTC::new();
        let mut data = [0; 4];

        // Read the current value of AMBA_ID_LOW.
        rtc.read(AMBA_ID_LOW, &mut data);
        assert_eq!(data[0], AMBA_IDS[0]);

        // Attempts to write to read-only registers (AMBA_ID_LOW in this case)
        // should have no effect.
        data = 123u32.to_le_bytes();
        rtc.write(AMBA_ID_LOW, &data);

        // Reread the current value of AMBA_ID_LOW and confirm it's unchanged.
        rtc.read(AMBA_ID_LOW, &mut data);
        assert_eq!(data[0], AMBA_IDS[0]);

        // Reading from the AMBA_ID registers should succeed.
        // Becuase we compute the index of the AMBA_IDS array by a logical bit
        // shift of (offset - AMBA_ID_LOW) >> 2, we want to make sure that
        // we correctly align down to a 4-byte register boundary, and that we
        // don't overflow (we shouldn't, since offset provided to read()
        // is unsigned).

        // Verify that we can read from AMBA_ID_LOW and that the logical shift
        // doesn't overflow.
        data = [0; 4];
        rtc.read(AMBA_ID_LOW, &mut data);
        assert_eq!(data[0], AMBA_IDS[0]);

        // Verify that attempts to read from AMBA_ID_LOW + 5 align down to
        // AMBA_ID_LOW + 4, corresponding to AMBA_IDS[1].
        data = [0; 4];
        rtc.read(AMBA_ID_LOW + 5, &mut data);
        assert_eq!(data[0], AMBA_IDS[1]);
    }

    #[test]
    fn test_invalid_write_offset() {
        // Test that writing to an invalid register offset has no effect
        // on the RTC value (as read from the data register).
        let mut rtc = RTC::new();
        let mut data = [0; 4];

        // First test: Write to an address outside the expected range of
        // register memory.

        // Read the data register.
        rtc.read(RTCDR, &mut data);
        let first_read = u32::from_le_bytes(data);

        // Attempt to write to an address outside the expected range of
        // register memory.
        data = 123u32.to_le_bytes();
        rtc.write(AMBA_ID_HIGH + 4, &mut data);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let second_read = u32::from_le_bytes(data);

        // RTCDR should be unchanged.
        // Note that this assumes less than 1 second has elapsed between
        // the first and second read of RTCDR (based on the RTC counter
        // tick rate being 1Hz).
        assert_eq!(second_read, first_read);

        // Second test: Attempt to write to a register address similar to the
        // load register, but not actually valid.

        // Read the data register.
        rtc.read(RTCDR, &mut data);
        let first_read = u32::from_le_bytes(data);

        // Attempt to write to an invalid register address close to the load
        // register's address.
        data = 123u32.to_le_bytes();
        rtc.write(RTCLR + 1, &mut data);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let second_read = u32::from_le_bytes(data);

        // RTCDR should be unchanged
        // Note that this assumes less than 1 second has elapsed between
        // the first and second read of RTCDR (based on the RTC counter
        // tick rate being 1Hz).
        assert_eq!(second_read, first_read);
    }

    #[test]
    fn test_invalid_read_offset() {
        let mut rtc = RTC::new();
        let mut data: [u8; 4];

        // Reading from a non-existent register should have no effect.
        data = 123u32.to_le_bytes();
        rtc.read(AMBA_ID_HIGH + 4, &mut data);
        assert_eq!(123, u32::from_le_bytes(data));

        // Just to prove that AMBA_ID_HIGH + 4 doesn't contain 123...
        data = 321u32.to_le_bytes();
        rtc.read(AMBA_ID_HIGH + 4, &mut data);
        assert_eq!(321, u32::from_le_bytes(data));
    }
}
