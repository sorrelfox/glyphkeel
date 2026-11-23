// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./styles/tokens.css";
import "./styles/components.css";
import "./styles/states.css";
import "./styles/base.css";

import { App } from "./App";
import { apply, DEFAULT_APPEARANCE, readDesktop, resolve } from "./appearance/appearance";
import { suppressBrowserBehaviour } from "./native";
import { readStart } from "./start";

// The window is still hidden. The theme goes onto <html> before the first
// render, so the first frame the user sees is already in it.
apply(resolve(DEFAULT_APPEARANCE, readDesktop(window)), document.documentElement);
suppressBrowserBehaviour(window);

const root = document.getElementById("root");
if (!root) {
  throw new Error("the page has no #root element");
}

createRoot(root).render(
  <StrictMode>
    <App start={readStart()} />
  </StrictMode>,
);
