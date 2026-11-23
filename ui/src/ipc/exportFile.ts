// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Saving an export the page made. In the app, Rust shows the system's save
// dialog and writes the file; the page never names a path. In a plain
// browser (the development bridge, the sample data) the browser downloads it.

import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { inApp } from "./files";

/** Saves `contents` as a file starting with the name `name`; whether it was saved. */
export async function saveExport(name: string, contents: string, type: string): Promise<boolean> {
  if (inApp()) {
    try {
      return await invoke<boolean>("save_export", { name, contents });
    } catch (error: unknown) {
      throw new OpFailure("save_export", toCallError(error));
    }
  }
  const url = URL.createObjectURL(new Blob([contents], { type }));
  try {
    const link = document.createElement("a");
    link.href = url;
    link.download = name;
    link.hidden = true;
    document.body.append(link);
    link.click();
    link.remove();
  } finally {
    // The download has its own copy once it starts.
    setTimeout(() => {
      URL.revokeObjectURL(url);
    }, 1000);
  }
  return true;
}
