// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/// <reference types="vite/client" />

/** True only in `vite --mode bridge`: the page may then talk to the development bridge. */
declare const __BRIDGE__: boolean;
