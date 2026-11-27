// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Exporting to an archive and importing one. In the app, Rust shows the
// native file chooser and runs the operation with the chosen path: the page
// names projects and a token for the chosen archive, never a path. In a
// plain browser (the development bridge, the sample data) an export goes to
// the workspace's `exports` folder, and an import names an archive there.

import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { type Call, type CsvImportInput, type CsvImportResult, type ExportResult, type ImportResult, type KeyChange, type WikiExportResult, type WikiImportResult } from "../generated/ops";
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
  /** Encrypt the export (a `.zip` file); absent: the engine's default, on for an encrypted workspace. */
  encrypt?: boolean;
}

/** Exports where the person chooses; null when they cancelled the chooser. */
export function exportArchive(call: Call, request: ExportRequest): Promise<ExportResult | null> {
  if (choosesFiles()) {
    return command<ExportResult | null>("export_archive", { projects: [...request.projects], folder: request.folder, name: request.name, encrypt: request.encrypt ?? null });
  }
  return call("transfer.export", { projects: [...request.projects], format: request.folder ? "folder" : "zip", ...(request.encrypt === undefined ? {} : { encrypt: request.encrypt }) });
}

/**
 * What to export from a wiki: a page by its ID (with the pages inside it
 * when `subpages`), or a project's whole wiki; the name the chooser
 * suggests; and whether as web pages instead of Markdown files.
 */
export type PageExportRequest = ({ page: string; subpages: boolean } | { project: string }) & { name: string; html?: boolean };

/** Exports a wiki page, or a whole wiki, as Markdown files or web pages in a `.zip` file where the person chooses; null when they cancelled the chooser. */
export function exportWikiPage(call: Call, request: PageExportRequest): Promise<WikiExportResult | null> {
  const input = "page" in request ? { page: request.page, subpages: request.subpages } : { project: request.project };
  const html = request.html === true;
  if (choosesFiles()) {
    return command<WikiExportResult | null>("export_wiki_page", { page: null, project: null, subpages: false, ...input, name: request.name, html });
  }
  return call("wiki.export", { ...input, text_format: html ? "html" : "markdown" });
}

/** Lets the person choose a folder of Markdown files to import into a wiki; null when they cancelled. Only in the app. */
export function chooseWikiFolder(): Promise<ImportSource | null> {
  return command<{ token: string; name: string } | null>("choose_wiki_folder", {});
}

/** Where a folder import goes: the project's wiki, inside a page (its ID) or at the top. */
export interface WikiImportTarget {
  project: string;
  parent?: string | undefined;
}

/** Checks (`dryRun`) or imports a chosen folder of Markdown files as wiki pages. */
export function importWikiFolder(call: Call, source: ImportSource, target: WikiImportTarget, dryRun: boolean): Promise<WikiImportResult> {
  if ("token" in source) {
    return command<WikiImportResult>("import_wiki_folder", { token: source.token, project: target.project, parent: target.parent ?? null, dryRun });
  }
  return call("wiki.import", { project: target.project, path: source.path, ...(target.parent === undefined ? {} : { parent: target.parent }), dry_run: dryRun });
}

/** An archive chosen for an import: in the app, its token; in a browser, its name in the exports folder. */
export type ImportSource = { token: string; name: string } | { path: string; name: string };

/** Lets the person choose an archive (a `.zip` file, or a folder); null when they cancelled. */
export function chooseImport(folder: boolean): Promise<ImportSource | null> {
  return command<{ token: string; name: string } | null>("choose_import", { folder });
}

/** Whether a failure asks for the passphrase of an encrypted export. */
export function wantsExportPassphrase(error: unknown): boolean {
  return error instanceof OpFailure && error.error.code !== "ipc" && error.error.details?.passphrase === "export";
}

/**
 * Checks (`dryRun`) or imports a chosen archive, with new keys for taken
 * ones. `passphrase` opens an encrypted export; the app keeps it in memory
 * until it quits. Only the app takes one.
 */
export function importArchive(call: Call, source: ImportSource, dryRun: boolean, keys: readonly KeyChange[], passphrase?: string): Promise<ImportResult> {
  if ("token" in source) {
    return command<ImportResult>("import_archive", { token: source.token, dryRun, keys: [...keys], passphrase: passphrase ?? null });
  }
  return call("transfer.import", { path: source.path, dry_run: dryRun, keys: [...keys] });
}

/** Lets the person choose a CSV file to import tasks from; null when they cancelled. Only in the app. */
export function chooseCsv(): Promise<ImportSource | null> {
  return command<{ token: string; name: string } | null>("choose_csv", {});
}

/** What a CSV import takes, without the file: that comes from the chosen source. */
export type CsvImportRequest = Omit<CsvImportInput, "path">;

/** Checks (`dry_run`) or runs a CSV import of a chosen file. */
export function importCsv(call: Call, source: ImportSource, request: CsvImportRequest): Promise<CsvImportResult> {
  if ("token" in source) {
    return command<CsvImportResult>("import_csv", { token: source.token, input: request });
  }
  return call("transfer.import_csv", { ...request, path: source.path });
}

/** Lets the person choose the backup folder in the native chooser; false when they cancelled. Only in the app. */
export async function chooseBackupFolder(): Promise<boolean> {
  return (await command<unknown>("choose_backup_folder", {})) !== null;
}

/** Sends backups to the default folder again. */
export async function resetBackupFolder(call: Call): Promise<void> {
  await call("backup.folder.update", {});
}

/** The chosen backup folder in the workspace's settings; absent while the default is used. */
export function chosenBackupFolder(settings: object | undefined): string | undefined {
  const folder = (settings as { backup_folder?: unknown } | undefined)?.backup_folder;
  return typeof folder === "string" && folder !== "" ? folder : undefined;
}
