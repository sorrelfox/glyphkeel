// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The pieces of the desktop shell that only one platform has. What they
//! show and decide lives in the shared modules; these only reach the system.

#[cfg(windows)]
pub(crate) mod windows;
