// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { lazy, Suspense, useEffect, useState } from "react";

import { AppearanceProvider } from "./appearance/AppearanceProvider";
import { I18n } from "./i18n/I18n";
import { windowReady, type Transport } from "./ipc/ready";
import { Shell } from "./shell/Shell";
import { StartupFailure } from "./shell/StartupFailure";
import { readStart, type StartData } from "./start";

// Loaded on first use, so that it costs the app's start nothing.
const Gallery = lazy(() => import("./dev/Gallery"));

/** The screens of the window. Navigation is app state, not URLs. */
export type Route = "app" | "gallery";

/** The path that opens the gallery directly, for the browser tests. */
export const GALLERY_PATH = "/dev/themes";

export function initialRoute(pathname: string): Route {
  return pathname.startsWith(GALLERY_PATH) ? "gallery" : "app";
}

interface AppProps {
  start?: StartData;
}

export function App({ start = readStart() }: AppProps) {
  return (
    <AppearanceProvider>
      <I18n locale={start.locale}>
        <Screens start={start} />
      </I18n>
    </AppearanceProvider>
  );
}

function Screens({ start }: { start: StartData }) {
  const [route, setRoute] = useState<Route>(() => initialRoute(window.location.pathname));
  const [transport, setTransport] = useState<Transport | null>(null);

  // React's first commit: the window may be shown now. The answer also says
  // whether the page's calls travel the way they should.
  useEffect(() => {
    windowReady().then(setTransport, (error: unknown) => {
      console.error("the app did not answer the first-screen signal", error);
    });
  }, []);

  if (transport === "postMessage") {
    return <StartupFailure reason="startup-ipc-fallback" />;
  }
  if (route === "gallery") {
    return (
      <Suspense>
        <Gallery onLeave={() => { setRoute("app"); }} />
      </Suspense>
    );
  }
  return <Shell titleBar={start.titleBar} openGallery={import.meta.env.DEV ? () => { setRoute("gallery"); } : undefined} />;
}
