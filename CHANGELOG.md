# Changelog

## [Unreleased]

## Added

- Added emulation support for an i8042 controller that only handles the CPU
  reset.

## Changed

- Changed the notification mechanism from EventFd to the Trigger abstraction
  for both serial console and i8042
  ([#7](https://github.com/rust-vmm/vm-superio/issues/7)).

## Fixed

- Limited the maximum number of bytes allowed at a time, when enqueuing input
  for serial, to 64 (FIFO_SIZE) to avoid memory pressure
  ([#17](https://github.com/rust-vmm/vm-superio/issues/17)).

# v0.1.0

This is the first vm-superio release.
The `vm-superio` crate provides emulation for legacy devices. For now, it offers
this support only for the Linux serial console.
