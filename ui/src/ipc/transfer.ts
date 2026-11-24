// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Exporting to an archive and importing one. In the app, Rust shows the
// native file chooser and runs the operation with the chosen path: the page
// names projects and a token for the chosen archive, never a path. In a
// plain browser (the development bridge, the sample data) an export goes to
// the workspace's `exports` folder, and an import names an archive there.

import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { type Call, type ExportResult, type ImportResult, type KeyChange } from "../generated/ops";
import { inApp } from "./files";

/** Runs an app command; its refusals become failures like the engine's. */
async function command<T>(name: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error: unknown) {
    throw new OpFailure(name, toCallError(error));
  }
}

/** Whether the native file chooser picks where archives go and come from. */
export function choosesFiles(): boolean {
  return inApp();
}

/** What to export: the projects (all when empty), and a `.zip` file or a folder. */
export interface ExportRequest {
  projects: readonly string[];
  folder: boolean;
  /** The name the chooser suggests. */
  name: string;
}

/** Exports where the person chooses; null when they cancelled the chooser. */
export function exportArchive(call: Call, request: ExportRequest): Promise<ExportResult | null> {
  if (choosesFiles()) {
    return command<ExportResult | null>("export_archive", { projects: [...request.projects], folder: request.folder, name: request.name });
  }
  return call("transfer.export", { projects: [...request.projects], format: request.folder ? "folder" : "zip" });
}

/** An archive chosen for an import: in the app, its token; in a browser, its name in the exports folder. */
export type ImportSource = { token: string; name: string } | { path: string; name: string };

/** Lets the person choose an archive (a `.zip` file, or a folder); null when they cancelled. */
export function chooseImport(folder: boolean): Promise<ImportSource | null> {
  return command<{ token: string; name: string } | null>("choose_import", { folder });
}

/** Checks (`dryRun`) or imports a chosen archive, with new keys for taken ones. */
export function importArchive(call: Call, source: ImportSource, dryRun: boolean, keys: readonly KeyChange[]): Promise<ImportResult> {
  if ("token" in source) {
    return command<ImportResult>("import_archive", { token: source.token, dryRun, keys: [...keys] });
  }
  return call("transfer.import", { path: source.path, dry_run: dryRun, keys: [...keys] });
}
