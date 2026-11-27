// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/// <reference types="vite/client" />

/** True only in `vite --mode bridge`: the page may then talk to the development bridge. */
declare const __BRIDGE__: boolean;

/** True only in `vite --mode fake`: the page then works on sample data in memory, without the engine. */
declare const __FAKE__: boolean;

/** The first and last port the development bridge may listen on (GK_PORT_RANGE at build time). */
declare const __BRIDGE_PORTS__: readonly [number, number];

/** The product's version, from the workspace's Cargo.toml. */
declare const __VERSION__: string;
