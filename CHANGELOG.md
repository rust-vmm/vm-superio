# Changelog

# v0.2.0

## Added

- Added emulation support for an i8042 controller that only handles the CPU
  reset ([#11](https://github.com/rust-vmm/vm-superio/pull/11)).
- Added `SerialEvents` trait, which can be implemented by a backend that wants
  to keep track of serial events using metrics, logs etc
  ([#5](https://github.com/rust-vmm/vm-superio/issues/5)).
- Added a threat model to the serial console documentation
  ([#16](https://github.com/rust-vmm/vm-superio/issues/16)).
- Added emulation support for an ARM PL031 Real Time Clock
  ([#22](https://github.com/rust-vmm/vm-superio/issues/22)), and the `RTCEvents`
  trait, used for keeping track of RTC events
  ([#34](https://github.com/rust-vmm/vm-superio/issues/34)).
- Added an implementation for `Arc<EV>` for both serial console and RTC device
  ([#40](https://github.com/rust-vmm/vm-superio/pull/40)).
- Added methods for retrieving a reference to the events object for both serial
  console and RTC device
  ([#40](https://github.com/rust-vmm/vm-superio/pull/40)).

## Changed

- Changed the notification mechanism from EventFd to the Trigger abstraction
  for both serial console and i8042
  ([#7](https://github.com/rust-vmm/vm-superio/issues/7)).

## Fixed

- Limited the maximum number of bytes allowed at a time, when enqueuing input
  for serial, to 64 (FIFO_SIZE) to avoid memory pressure
  ([#17](https://github.com/rust-vmm/vm-superio/issues/17)).
- Fixed possible indefinite blocking of the serial driver by always sending the
  THR Empty interrupt to it when trying to write to the device
  ([#23](https://github.com/rust-vmm/vm-superio/issues/23)).

# v0.1.0

This is the first vm-superio release.
The `vm-superio` crate provides emulation for legacy devices. For now, it offers
this support only for the Linux serial console.
