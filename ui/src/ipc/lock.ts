// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The app lock and the workspace's encryption.
//
// The app keeps the lock: while it is locked it refuses every call, and the
// page shows the lock screen instead of the app. It unlocks with the
// workspace's passphrase when the workspace is encrypted, else with a
// passphrase kept for the lock alone. The same screen asks for the
// passphrase of an encrypted workspace the app could not open at start.
//
// Outside the app (the sample data, tests) the page keeps a stand-in in
// memory: a plain workspace that can be "encrypted", a lock passphrase, and
// the idle time. Nothing of it is stored.

import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { inApp } from "./files";

export interface EncryptionStatus {
  /** This build can encrypt. */
  available: boolean;
  /** The workspace is encrypted. */
  encrypted: boolean;
}

export interface LockView {
  /** The app is locked, or the workspace waits for its passphrase. */
  locked: boolean;
  /** `app`: the app lock; `workspace`: an encrypted workspace that didn't open yet. */
  reason?: "app" | "workspace";
  /** There is a passphrase to unlock with, so the app can lock. */
  can_lock: boolean;
  /** Minutes of idle time before the app locks; absent: never. */
  lock_after_minutes?: number;
  /** A passphrase is kept for the lock of a plain workspace. */
  lock_passphrase: boolean;
  /** The workspace's encryption; absent without a workspace. */
  encryption?: EncryptionStatus;
}

export interface EncryptionChanged extends LockView {
  /** The backup made before the change. */
  backup: string;
  /** Backups that stayed unencrypted. */
  backups_left_plain: number;
}

export interface LockPatch {
  /** Minutes of idle time before the app locks; 0 for never. */
  lock_after_minutes?: number;
  /** A new passphrase for the lock; empty removes it. */
  lock_passphrase?: string;
  /** The lock's passphrase now, to change or remove it. */
  current?: string;
}

export interface EncryptionRequest {
  /** Encrypt (or give a new passphrase); false: store it unencrypted again. */
  encrypt: boolean;
  /** The new passphrase. */
  passphrase?: string;
  /** The passphrase now, for an encrypted workspace. */
  current?: string;
}

// ---------- The stand-in outside the app ----------

interface Fake {
  locked: boolean;
  lockPassphrase: string | null;
  workspacePassphrase: string | null;
  after: number | null;
}

/** Why the stand-in is locked: always the app lock. */
const APP_LOCK = "app" as const;

const fake: Fake = { locked: false, lockPassphrase: null, workspacePassphrase: null, after: null };

function fakeView(): LockView {
  const encrypted = fake.workspacePassphrase !== null;
  return {
    locked: fake.locked,
    ...(fake.locked ? { reason: APP_LOCK } : {}),
    can_lock: encrypted || fake.lockPassphrase !== null,
    ...(fake.after === null ? {} : { lock_after_minutes: fake.after }),
    lock_passphrase: fake.lockPassphrase !== null,
    encryption: { available: true, encrypted },
  };
}

function refuse(op: string, messageId: string, field = "passphrase"): Promise<never> {
  return Promise.reject(
    new OpFailure(op, {
      code: "invalid",
      message: messageId,
      details: { message_id: messageId, path: field, ...(messageId === "lock-passphrase-wrong" ? { passphrase: "wrong" } : {}) },
    }),
  );
}

/** Resets the stand-in; for tests. */
export function resetFakeLock(): void {
  Object.assign(fake, { locked: false, lockPassphrase: null, workspacePassphrase: null, after: null });
}

// ---------- Calls ----------

async function command<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error: unknown) {
    throw new OpFailure(name, toCallError(error));
  }
}

export function loadLock(): Promise<LockView> {
  return inApp() ? command("lock_state") : Promise.resolve(fakeView());
}

/** Locks the app now. */
export function lockApp(): Promise<LockView> {
  if (inApp()) {
    return command("lock_app");
  }
  if (!fakeView().can_lock) {
    return refuse("lock_app", "lock-not-possible");
  }
  fake.locked = true;
  return Promise.resolve(fakeView());
}

/** Unlocks with `passphrase`, or opens the encrypted workspace that waits for it. */
export function unlockApp(passphrase: string): Promise<LockView> {
  if (inApp()) {
    return command("unlock_app", { passphrase });
  }
  if (passphrase === "") {
    return refuse("unlock_app", "lock-passphrase-empty");
  }
  const right = fake.workspacePassphrase ?? fake.lockPassphrase;
  if (passphrase !== right) {
    return refuse("unlock_app", "lock-passphrase-wrong");
  }
  fake.locked = false;
  return Promise.resolve(fakeView());
}

/** The person did something; answers whether the app is locked. */
export function reportActivity(): Promise<boolean> {
  return inApp() ? command("app_activity") : Promise.resolve(fake.locked);
}

export function updateLock(patch: LockPatch): Promise<LockView> {
  if (inApp()) {
    return command("update_lock", { patch });
  }
  if (patch.lock_passphrase !== undefined) {
    if (fake.lockPassphrase !== null && patch.current !== fake.lockPassphrase) {
      return refuse("update_lock", "lock-passphrase-wrong", "current");
    }
    fake.lockPassphrase = patch.lock_passphrase === "" ? null : patch.lock_passphrase;
  }
  if (patch.lock_after_minutes !== undefined) {
    fake.after = patch.lock_after_minutes > 0 ? patch.lock_after_minutes : null;
  }
  return Promise.resolve(fakeView());
}

export function changeEncryption(request: EncryptionRequest): Promise<EncryptionChanged> {
  if (inApp()) {
    return command("change_encryption", { ...request });
  }
  if (fake.workspacePassphrase !== null && request.current !== fake.workspacePassphrase) {
    return refuse("change_encryption", "lock-passphrase-wrong", "current");
  }
  if (request.encrypt) {
    if (request.passphrase === undefined || request.passphrase === "") {
      return refuse("change_encryption", "lock-passphrase-empty");
    }
    fake.workspacePassphrase = request.passphrase;
  } else {
    fake.workspacePassphrase = null;
  }
  return Promise.resolve({ ...fakeView(), backup: "2026-11-27T120000Z", backups_left_plain: 0 });
}

// ---------- Hooks ----------

export const LOCK_KEY = ["app-lock"] as const;

/**
 * The lock, looked at again every few seconds: another window may have
 * locked the app, or the idle time run out.
 */
export function useLock(): UseQueryResult<LockView, OpFailure> {
  return useQuery({ queryKey: LOCK_KEY, queryFn: loadLock, refetchInterval: 5000, staleTime: 1000 });
}

/** A call that changes the lock; its answer replaces the lock's state. */
export function useLockMutation<A>(run: (args: A) => Promise<LockView>) {
  const queryClient = useQueryClient();
  return useMutation<LockView, OpFailure, A>({
    mutationFn: run,
    onSuccess: (view) => {
      const { locked, reason, can_lock, lock_after_minutes, lock_passphrase, encryption } = view;
      queryClient.setQueryData<LockView>(LOCK_KEY, {
        locked,
        ...(reason === undefined ? {} : { reason }),
        can_lock,
        ...(lock_after_minutes === undefined ? {} : { lock_after_minutes }),
        lock_passphrase,
        ...(encryption === undefined ? {} : { encryption }),
      });
    },
  });
}
