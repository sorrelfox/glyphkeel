// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, type ReactNode } from "react";

import { LOCK_KEY, lockApp, reportActivity, useLock, useLockMutation } from "../../ipc/lock";
import { LockScreen } from "./LockScreen";

/** How often the app hears that the person is active, at most. */
const ACTIVITY_EVERY_MS = 30_000;
/** How often the idle time is looked at. */
const IDLE_CHECK_MS = 5_000;
const ACTIVITY_EVENTS = ["pointerdown", "keydown", "wheel", "touchstart"] as const;

/**
 * Shows the app, or the lock screen while it is locked. Locking drops every
 * answer the page keeps, so nothing of the workspace stays rendered or
 * cached; unlocking reads it again. The person's activity keeps the idle
 * time from running out; when it does, the app locks.
 */
export function LockGate({ children }: { children: ReactNode }) {
  const lock = useLock();
  const queryClient = useQueryClient();
  const locked = lock.data?.locked === true;
  const lockNow = useLockMutation(() => lockApp());
  const lastActivity = useRef(0);
  const lastReport = useRef(0);

  useEffect(() => {
    if (locked) {
      queryClient.removeQueries({ predicate: (query) => query.queryKey[0] !== LOCK_KEY[0] });
    }
  }, [locked, queryClient]);

  const minutes = lock.data?.can_lock === true ? lock.data.lock_after_minutes : undefined;
  const { mutate } = lockNow;
  useEffect(() => {
    if (locked) {
      return undefined;
    }
    // The idle time starts when the app shows (again).
    lastActivity.current = Date.now();
    const onActivity = () => {
      const now = Date.now();
      lastActivity.current = now;
      if (now - lastReport.current >= ACTIVITY_EVERY_MS) {
        lastReport.current = now;
        void reportActivity().then((isLocked) => {
          if (isLocked) {
            void queryClient.invalidateQueries({ queryKey: LOCK_KEY });
          }
        });
      }
    };
    for (const name of ACTIVITY_EVENTS) {
      window.addEventListener(name, onActivity, { capture: true, passive: true });
    }
    const timer =
      minutes === undefined
        ? undefined
        : window.setInterval(() => {
            if (Date.now() - lastActivity.current >= minutes * 60_000) {
              mutate(undefined);
            }
          }, IDLE_CHECK_MS);
    return () => {
      for (const name of ACTIVITY_EVENTS) {
        window.removeEventListener(name, onActivity, { capture: true });
      }
      if (timer !== undefined) {
        window.clearInterval(timer);
      }
    };
  }, [locked, minutes, mutate, queryClient]);

  if (lock.data?.locked === true) {
    return <LockScreen view={lock.data} />;
  }
  return <>{children}</>;
}
