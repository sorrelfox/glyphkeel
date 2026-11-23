// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { isTauri } from "@tauri-apps/api/core";

import { tauriTransport } from "./tauri";
import { createClient, deferredTransport, unavailableTransport, type Client, type Transport } from "./transport";

/** Why a page has no engine: opened in a plain browser, without the bridge. */
export const NO_ENGINE = "this page runs outside the app, without an engine";

/**
 * The transport of this page: the app's IPC inside the app; in a
 * development build for the bridge, the bridge named in the URL; otherwise
 * none. The bridge's code exists only in that build.
 */
export function connect(): Transport {
  if (isTauri()) {
    return tauriTransport();
  }
  if (__BRIDGE__) {
    return deferredTransport(
      import("./bridge").then(({ bridgeAddress, bridgeTransport }) => {
        const address = bridgeAddress(window.location.search);
        return address ? bridgeTransport(address) : unavailableTransport(NO_ENGINE);
      }),
    );
  }
  return unavailableTransport(NO_ENGINE);
}

let pageClient: Client | undefined;

/** The page's one client, made on first use. */
export function defaultClient(): Client {
  pageClient ??= createClient(connect());
  return pageClient;
}
