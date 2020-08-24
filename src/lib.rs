// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Emulation for legacy devices.
//!
//! For now, it offers emulation support only for the Linux serial console.

#![deny(missing_docs)]

pub mod serial;

pub use serial::Serial;
