// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Types of the rule lists the lint test reads from eslint.config.js.

import type { Linter } from "eslint";

type Restriction = { selector: string; message: string };

export const joinedText: Restriction[];
export const physicalStyles: Restriction[];

declare const config: Linter.Config[];
export default config;
