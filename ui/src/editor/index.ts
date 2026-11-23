// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The Markdown editor, loaded on first use: CodeMirror and everything it
 * brings are a chunk of their own, so the app's start pays nothing for them.
 * KaTeX, Mermaid and the HTML paste converter load later still, on the first
 * formula, diagram or HTML paste.
 */

import { lazy } from "react";

export const MarkdownEditor = lazy(() => import("./MarkdownEditor"));

export type {
  BlockRenderer,
  ChecklistToggle,
  CompletionProviders,
  EditorMode,
  LineBreakNote,
  MarkdownEditorHandle,
  MarkdownEditorProps,
  RenderedBlockKind,
  SpellChecker,
  TaskSuggestion,
  TextEdit,
} from "./types";
