// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { RouterProvider } from "@tanstack/react-router";
import { useState } from "react";

import { AppearanceProvider } from "./appearance/AppearanceProvider";
import { ToastProvider } from "./components/Toasts";
import { defaultClient } from "./data/connect";
import { DataProvider } from "./data/DataProvider";
import { type Client } from "./data/transport";
import { I18n } from "./i18n/I18n";
import { loadAppearance, saveAppearance } from "./prefs/store";
import { createAppRouter, initialPath } from "./router";
import { readStart, type StartData } from "./start";

interface AppProps {
  start?: StartData;
  /** The engine's client; the page's own unless a test passes one. */
  client?: Client;
  /** The first screen; by default from the page's path. */
  path?: string;
}

export function App({ start = readStart(), client = defaultClient(), path }: AppProps) {
  const [router] = useState(() => createAppRouter(path ?? initialPath(window.location.pathname, start), { start }));
  const [appearance] = useState(loadAppearance);
  return (
    <AppearanceProvider initial={appearance} onChange={saveAppearance}>
      <I18n locale={start.locale}>
        <ToastProvider>
          <DataProvider client={client}>
            <RouterProvider router={router} />
          </DataProvider>
        </ToastProvider>
      </I18n>
    </AppearanceProvider>
  );
}
