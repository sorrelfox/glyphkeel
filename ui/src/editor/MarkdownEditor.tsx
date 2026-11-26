// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { EditorState, Transaction } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { X } from "lucide-react";
import { useEffect, useImperativeHandle, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { Button } from "react-aria-components";

import { useContextMenu, type MenuEntry } from "../components/ContextMenu";
import { classes } from "../classes";
import { useT } from "../i18n/I18n";
import { webViewChecksSpelling } from "../platform";
import { startLink } from "./complete";
import { applyFormat } from "./format";
import { convertLineBreaks, openText, type LineBreak } from "./lineEndings";
import styles from "./MarkdownEditor.module.css";
import { filePasting, insertMarkdown } from "./paste";
import { applyTextEdits, compartments, editorExtensions, labelExtension, modeExtension, readOnlyExtension, rendererExtension, spellingExtension } from "./setup";
import { defaultSpellChecker, misspeltAt } from "./spell";
import type { LineBreakNote, MarkdownEditorHandle, MarkdownEditorProps } from "./types";

const LINE_BREAK_NAMES: Record<LineBreak, string> = {
  "\n": "editor-line-break-lf",
  "\r\n": "editor-line-break-crlf",
  "\r": "editor-line-break-cr",
};

/**
 * The Markdown editor of descriptions, comments and wiki pages: CodeMirror
 * with Live Preview (or the plain source), completion, slash commands, HTML
 * paste and spelling. The text is kept exactly: `onChange` reports it byte
 * for byte, with the text's own line breaks.
 */
export default function MarkdownEditor(props: MarkdownEditorProps) {
  const { value, mode = "preview", readOnly = false, label, className, autoFocus = false, ref } = props;
  const t = useT();
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  // The text the editor last reported or was given, to tell outside changes from echoes.
  const known = useRef<string>("");
  const latest = useRef(props);
  const translate = useRef(t);
  const [note, setNote] = useState<LineBreakNote | null>(null);
  const spellChecker = props.spellChecker === undefined ? defaultSpellChecker() : props.spellChecker;
  // Where the web view checks spelling itself, a screen that passes no checker gets the web view's.
  const webViewSpelling = props.spellChecker === undefined && webViewChecksSpelling();
  const renderBlock = props.renderBlock ?? null;
  const spellMenu = useContextMenu(t("editor-spelling-menu"));

  useEffect(() => {
    latest.current = props;
    translate.current = t;
  });

  /** Opens a text: picks its line break, converting mixed ones once. */
  const open = (text: string): EditorState => {
    const opened = openText(text);
    known.current = opened.text;
    if (opened.converted) {
      const converted = opened.converted;
      setNote(converted);
      latest.current.onLineBreaksConverted?.(converted);
      // The converted text is the text now; the screen holds the same.
      queueMicrotask(() => latest.current.onChange?.(opened.text));
    }
    return EditorState.create({
      doc: opened.text,
      extensions: editorExtensions({
        lineBreak: opened.lineBreak,
        mode,
        readOnly,
        spellChecker,
        webViewSpelling,
        renderBlock,
        label,
        environment: {
          providers: () => latest.current.completion ?? {},
          text: (id) => translate.current(id),
          toggle: () => latest.current.onToggleChecklist ?? null,
        },
        extensions: [
          filePasting(() => latest.current.onFiles ?? null),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) {
              return;
            }
            const text = update.state.sliceDoc();
            known.current = text;
            // A value set from outside isn't reported back.
            if (update.transactions.some((tr) => tr.docChanged && tr.annotation(Transaction.remote) !== true)) {
              latest.current.onChange?.(text);
            }
          }),
        ],
      }),
    });
  };

  // The view lives as long as the component.
  useEffect(() => {
    if (!host.current) {
      return undefined;
    }
    const editor = new EditorView({ parent: host.current, state: open(value) });
    view.current = editor;
    if (autoFocus) {
      editor.focus();
    }
    return () => {
      editor.destroy();
      view.current = null;
    };
    // Opened once; later values arrive through the effect below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A new value from outside: the smallest replacement, or a fresh start when the line break differs.
  useEffect(() => {
    const editor = view.current;
    if (!editor || value === known.current) {
      return;
    }
    const opened = openText(value);
    if (opened.lineBreak !== editor.state.lineBreak || opened.converted) {
      editor.setState(open(value));
      return;
    }
    known.current = value;
    const current = editor.state.doc.sliceString(0, editor.state.doc.length, "\n");
    const next = convertLineBreaks(value, "\n");
    let start = 0;
    const shorter = Math.min(current.length, next.length);
    while (start < shorter && current.charCodeAt(start) === next.charCodeAt(start)) {
      start++;
    }
    let endCurrent = current.length;
    let endNext = next.length;
    while (endCurrent > start && endNext > start && current.charCodeAt(endCurrent - 1) === next.charCodeAt(endNext - 1)) {
      endCurrent--;
      endNext--;
    }
    editor.dispatch({
      changes: { from: start, to: endCurrent, insert: convertLineBreaks(next.slice(start, endNext), editor.state.lineBreak) },
      annotations: [Transaction.addToHistory.of(false), Transaction.remote.of(true)],
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);

  useEffect(() => {
    view.current?.dispatch({ effects: compartments.mode.reconfigure(modeExtension(mode)) });
  }, [mode]);
  useEffect(() => {
    view.current?.dispatch({ effects: compartments.readOnly.reconfigure(readOnlyExtension(readOnly)) });
  }, [readOnly]);
  useEffect(() => {
    view.current?.dispatch({ effects: compartments.spelling.reconfigure(spellingExtension(spellChecker)) });
  }, [spellChecker]);
  useEffect(() => {
    view.current?.dispatch({ effects: compartments.renderer.reconfigure(rendererExtension(renderBlock)) });
  }, [renderBlock]);
  useEffect(() => {
    view.current?.dispatch({ effects: compartments.label.reconfigure(labelExtension(label, webViewSpelling)) });
  }, [label, webViewSpelling]);

  useImperativeHandle(
    ref,
    (): MarkdownEditorHandle => ({
      focus: () => view.current?.focus(),
      text: () => view.current?.state.sliceDoc() ?? known.current,
      applyEdits: (edits) => (view.current ? applyTextEdits(view.current, edits) : false),
      insert: (text, at) => {
        const editor = view.current;
        if (!editor) {
          return;
        }
        if (at !== undefined) {
          editor.dispatch({ selection: { anchor: Math.min(at, editor.state.doc.length) } });
        }
        insertMarkdown(editor, text, "input");
      },
      format: (kind) => {
        const editor = view.current;
        if (editor) {
          applyFormat(editor, kind);
          editor.focus();
        }
      },
      startLink: (kind) => {
        const editor = view.current;
        if (editor && !editor.state.readOnly) {
          startLink(editor, kind);
        }
      },
      positionAt: (x, y) => view.current?.posAtCoords({ x, y }) ?? null,
    }),
    [],
  );

  /** Right-click on a misspelt word: our menu with the spell checker's suggestions. */
  const onContextMenu = (event: ReactMouseEvent) => {
    const editor = view.current;
    if (!editor || !spellChecker || readOnly) {
      return;
    }
    const pos = editor.posAtCoords({ x: event.clientX, y: event.clientY });
    const word = pos === null ? null : misspeltAt(editor, pos);
    if (!word) {
      return;
    }
    event.preventDefault();
    const at = { clientX: event.clientX, clientY: event.clientY };
    const text = editor.state.sliceDoc(word.from, word.to);
    void spellChecker.suggest(text).then((suggestions) => {
      const entries: MenuEntry[] =
        suggestions.length === 0
          ? [{ id: "none", label: t("editor-no-suggestions"), run: () => undefined, disabled: true }]
          : suggestions.slice(0, 8).map((suggestion) => ({
              id: `suggestion-${suggestion}`,
              label: suggestion,
              run: () => {
                // Only if the word is still the same text in the same place.
                if (editor.state.sliceDoc(word.from, word.to) === text) {
                  editor.dispatch({ changes: { from: word.from, to: word.to, insert: suggestion }, userEvent: "input.spelling" });
                }
                editor.focus();
              },
            }));
      spellMenu.open(at, entries);
    });
  };

  return (
    <div className={classes(styles.editor, className)} data-mode={mode}>
      {note && (
        <div className={classes("banner", styles.note)} role="status">
          <span className="grow">{t("editor-line-breaks-converted", { kind: t(LINE_BREAK_NAMES[note.to]) })}</span>
          <Button
            className="btn sm icon flat"
            aria-label={t("editor-dismiss-note")}
            onPress={() => {
              setNote(null);
            }}
          >
            <X size={14} aria-hidden="true" />
          </Button>
        </div>
      )}
      <div ref={host} className={styles.host} onContextMenu={onContextMenu} />
      {spellMenu.menu}
    </div>
  );
}
