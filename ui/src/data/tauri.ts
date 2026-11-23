// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Channel, invoke } from "@tauri-apps/api/core";

import { type Changed } from "../generated/ops";
import { type Transport } from "./transport";

/**
 * The desktop app: every operation goes through the one `op` command, and the
 * change events arrive on a channel the window hands to `subscribe`.
 */
export function tauriTransport(): Transport {
  return {
    call: (op, input) => invoke("op", { op, input: input ?? {} }),
    async subscribe(onEvent) {
      let active = true;
      const channel = new Channel<Changed>((event) => {
        if (active) {
          onEvent(event);
        }
      });
      await invoke<boolean>("subscribe", { channel });
      // The app keeps one channel per window; a later subscription replaces
      // this one there. Here it only stops listening.
      return () => {
        active = false;
      };
    },
  };
}
