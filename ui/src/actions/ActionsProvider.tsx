// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { createContext, use, useEffect, type ReactNode } from "react";

import { dispatchKey, type Action } from "./actions";

const ActionsContext = createContext<readonly Action[]>([]);

interface ActionsProviderProps {
  actions: readonly Action[];
  children: ReactNode;
}

/**
 * Makes the actions available to menus and the shortcut sheet, and runs them
 * from the keyboard. Key presses that a focused component already handled
 * (a menu closing on Escape, say) are left alone.
 */
export function ActionsProvider({ actions, children }: ActionsProviderProps) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      dispatchKey(actions, event);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [actions]);

  return <ActionsContext value={actions}>{children}</ActionsContext>;
}

export function useActions(): readonly Action[] {
  return use(ActionsContext);
}

/** The action with this ID; it must be registered. */
export function useAction(id: string): Action {
  const action = useActions().find((candidate) => candidate.id === id);
  if (!action) {
    throw new Error(`no action "${id}"`);
  }
  return action;
}
