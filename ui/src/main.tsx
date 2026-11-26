// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./styles/tokens.css";
import "./styles/components.css";
import "./styles/states.css";
import "./styles/base.css";

import { App } from "./App";
import { apply, readDesktop, resolve } from "./appearance/appearance";
import { suppressBrowserBehaviour } from "./native";
import { applyPlatform } from "./platform";
import { loadAppearance } from "./prefs/store";
import { readStart } from "./start";

const start = readStart();
// The window is still hidden. The platform and the theme go onto <html>
// before the first render, so the first frame the user sees is already in them.
applyPlatform(start.platform ?? "linux", document.documentElement);
apply(resolve(loadAppearance(), readDesktop(window)), document.documentElement);
suppressBrowserBehaviour(window);

const root = document.getElementById("root");
if (!root) {
  throw new Error("the page has no #root element");
}

createRoot(root).render(
  <StrictMode>
    <App start={start} />
  </StrictMode>,
);
