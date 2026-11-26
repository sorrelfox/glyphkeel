// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Attached files and the desktop. In the app, Rust shows the file chooser,
// takes files dropped on the window, opens and saves files, and serves
// images through `gk-file:`; the page only ever names the task, never a
// path. Files belong to a task or to a wiki page. In a plain browser (the
// development bridge, the sample data, tests)
// files come from the page's own file field and drops, as Base64, and
// images are shown from their content.

import { Channel, convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";

import { type Call, type CallError, type FileChange } from "../generated/ops";
import { OpFailure, toCallError } from "../data/transport";

/** What an attach did: the files attached, and the ones that failed with why. */
export interface Attached {
  files: FileChange[];
  failed: { name: string; error: CallError }[];
}

/** What files are attached to: a task by its key, or a wiki page by its ID. */
export type FileOwner = string | { page: string };

/** The owner as the engine's and the app's inputs name it. */
export function ownerInput(owner: FileOwner): { task: string } | { page: string } {
  return typeof owner === "string" ? { task: owner } : { page: owner.page };
}

/** The owner as one string, for the elements that take a drop: the task's key, or `page:` and the page's ID. */
export function ownerKey(owner: FileOwner): string {
  return typeof owner === "string" ? owner : `page:${owner.page}`;
}

/** Whether the desktop app is there to show dialogs and open files. */
export function inApp(): boolean {
  return isTauri() && !__FAKE__;
}

/** Runs an app command; its refusals become failures like the engine's. */
async function command<T>(name: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error: unknown) {
    throw new OpFailure(name, toCallError(error));
  }
}

/** The address of an attached image for an `<img>`, served by the app. */
export function imageUrl(attachment: string): string {
  return convertFileSrc(attachment, "gk-file");
}

/** The address of an attached image's thumbnail (a PNG, at most 256 pixels a side), served by the app. */
export function thumbnailUrl(attachment: string): string {
  return `${imageUrl(attachment)}/thumb`;
}

/** Lets the person choose files in the system's dialog and attaches them to `owner`. */
export function chooseAndAttach(owner: FileOwner): Promise<Attached> {
  return command<Attached>("attach_files", ownerInput(owner));
}

/** Reads a file of the page's own as Base64. */
function base64Of(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const url = typeof reader.result === "string" ? reader.result : "";
      resolve(url.slice(url.indexOf(",") + 1));
    };
    reader.onerror = () => {
      reject(reader.error ?? new Error("the file can't be read"));
    };
    reader.readAsDataURL(file);
  });
}

/** Attaches files the page holds (from its file field or a drop in a browser), one call each. */
export async function attachBrowserFiles(call: Call, owner: FileOwner, files: readonly File[]): Promise<Attached> {
  const attached: Attached = { files: [], failed: [] };
  for (const file of files) {
    try {
      const data = await base64Of(file);
      attached.files.push(await call("file.attach", { ...ownerInput(owner), name: file.name, data_base64: data, client_request_id: crypto.randomUUID() }));
    } catch (error: unknown) {
      attached.failed.push({ name: file.name, error: error instanceof OpFailure ? error.error : toCallError(error) });
    }
  }
  return attached;
}

/** Opens an attached file in the desktop's default app. Executables are refused: save them. */
export function openAttachment(attachment: string): Promise<void> {
  return command<undefined>("open_file", { attachment });
}

/** Saves a copy of an attached file where the person chooses; whether it was saved. */
export function saveAttachment(attachment: string): Promise<boolean> {
  return command<boolean>("save_file", { attachment });
}

/** Files dragged over the window from the desktop, in CSS pixels. */
export type DesktopDrop =
  | { kind: "enter"; count: number; x: number; y: number }
  | { kind: "over"; x: number; y: number }
  | { kind: "leave" }
  | { kind: "drop"; id: number; names: string[]; x: number; y: number };

const dropListeners = new Set<(event: DesktopDrop) => void>();
let watching = false;

/**
 * Listens to files dragged from the desktop onto this window. The app keeps
 * one channel per window, so the page asks for it once.
 */
export function onDesktopDrop(listener: (event: DesktopDrop) => void): () => void {
  dropListeners.add(listener);
  if (!watching && inApp()) {
    watching = true;
    const channel = new Channel<DesktopDrop>((event) => {
      for (const each of dropListeners) {
        each(event);
      }
    });
    invoke("watch_drops", { channel }).catch((error: unknown) => {
      watching = false;
      console.error("the app did not take the drop channel", error);
    });
  }
  return () => {
    dropListeners.delete(listener);
  };
}

/** Attaches the files of a desktop drop to `owner`. */
export function attachDrop(drop: number, owner: FileOwner): Promise<Attached> {
  return command<Attached>("attach_drop", { drop, ...ownerInput(owner) });
}

/** What takes files dropped at a point of the window, if anything: a task's panel or a wiki page ({@link ownerKey}). */
export function dropTargetAt(x: number, y: number): string | null {
  const element = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop-task]");
  return element?.dataset.dropTask ?? null;
}
