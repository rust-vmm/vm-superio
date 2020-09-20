// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Emulation for legacy devices.
//!
//! For now, it offers emulation support only for the Linux serial console
//! and an i8042 PS/2 controller that only handles the CPU reset.

#![deny(missing_docs)]

pub mod i8042;
pub mod serial;

pub use i8042::I8042Device;
pub use serial::Serial;
