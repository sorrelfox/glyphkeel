// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./styles/tokens.css";
import "./styles/components.css";
import "./styles/base.css";

import { App } from "./App";
import { suppressBrowserBehaviour } from "./native";

suppressBrowserBehaviour(document);

const root = document.getElementById("root");
if (!root) {
  throw new Error("the page has no #root element");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
