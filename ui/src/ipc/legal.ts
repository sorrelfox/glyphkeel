// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The licence and the third-party notices, for Preferences › About. The app
// reads the copies its package installed; the page only says which text it
// wants. The development builds on sample data or against the bridge carry
// the repository's copies instead, so their screens can be tried; a release
// build has no such copy, and a plain browser shows where to find the files.

import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { inApp } from "./files";

export type LegalText = "licence" | "notices";

/** The file each text comes from, as the packages install it. */
export const LEGAL_FILES: Record<LegalText, string> = {
  licence: "LICENSE",
  notices: "THIRD-PARTY-NOTICES.txt",
};

/** The repository's copy, in development builds only; a release build drops this code. */
async function developmentCopy(which: LegalText): Promise<string | null> {
  if (!(__FAKE__ || __BRIDGE__)) {
    return null;
  }
  const module = which === "licence" ? await import("../../../LICENSE?raw") : await import("../../../THIRD-PARTY-NOTICES.txt?raw");
  return module.default;
}

async function read(which: LegalText): Promise<string | null> {
  if (!inApp()) {
    return developmentCopy(which);
  }
  try {
    return await invoke<string>("legal_text", { which });
  } catch (error: unknown) {
    throw new OpFailure("legal_text", toCallError(error));
  }
}

/** The text, once `enabled`; `null` where the page has no copy of it. */
export function useLegalText(which: LegalText, enabled: boolean): UseQueryResult<string | null, OpFailure> {
  return useQuery({ queryKey: ["legal-text", which], queryFn: () => read(which), enabled, staleTime: Infinity });
}
