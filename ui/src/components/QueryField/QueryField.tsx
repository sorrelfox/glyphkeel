// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { startCompletion } from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { Filter } from "lucide-react";
import { useEffect, useId, useImperativeHandle, useRef, type Ref } from "react";

import { classes } from "../../classes";
import { type QueryField as FieldInfo } from "../../generated/ops";
import { queryFieldExtensions, setReading, type QueryNames, type Reading } from "./queryField";
import styles from "./QueryField.module.css";

export type { QueryNames, Reading };

/** What a screen can do with the field. */
export interface QueryFieldHandle {
  focus: () => void;
  /** Adds a space at the end, puts the cursor there and opens the list of fields. */
  addCondition: () => void;
}

interface QueryFieldProps {
  /** The text. A change from outside (a saved view, a removed chip) replaces what is typed. */
  value: string;
  onChange: (text: string) => void;
  /** The engine's reading of the text: colours and the mistake. */
  reading: Reading | undefined;
  /** The mistake, in words, shown under the field. */
  problem?: string | undefined;
  fields: readonly FieldInfo[] | undefined;
  names: QueryNames;
  label: string;
  placeholder: string;
  /** Enter. */
  onSubmit?: () => void;
  /** Escape. */
  onEscape?: () => void;
  /** A shortcut shown at the end of the field. */
  shortcut?: React.ReactNode;
  /** The ID of a hint that describes the field, read with it. */
  describedBy?: string;
  ref?: Ref<QueryFieldHandle>;
}

/**
 * The one-line query field: CodeMirror with the engine's colours, the first
 * mistake underlined in red with its message below, and completion of
 * fields and their values.
 */
export function QueryField({ value, onChange, reading, problem, fields, names, label, placeholder, onSubmit, onEscape, shortcut, describedBy, ref }: QueryFieldProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const messageId = useId();
  // The editor reads these when it needs them, so it is made once.
  const latest = useRef({ onChange, onSubmit, onEscape, fields, names });
  useEffect(() => {
    latest.current = { onChange, onSubmit, onEscape, fields, names };
  });

  useEffect(() => {
    if (!host.current) {
      return;
    }
    const editor = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: value,
        extensions: queryFieldExtensions({
          label,
          placeholder,
          environment: { fields: () => latest.current.fields, names: () => latest.current.names },
          onSubmit: () => latest.current.onSubmit?.(),
          onEscape: () => latest.current.onEscape?.(),
          onText: (text) => {
            latest.current.onChange(text);
          },
        }),
      }),
    });
    view.current = editor;
    return () => {
      editor.destroy();
      view.current = null;
    };
    // The text arrives through the effect below; the editor stays.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [label, placeholder]);

  // A text from outside replaces the field's.
  useEffect(() => {
    const editor = view.current;
    if (editor && editor.state.doc.toString() !== value) {
      editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value }, selection: { anchor: value.length } });
    }
  }, [value]);

  useEffect(() => {
    if (reading && view.current) {
      view.current.dispatch({ effects: setReading.of(reading) });
    }
  }, [reading]);

  useImperativeHandle(ref, () => ({
    focus: () => view.current?.focus(),
    addCondition: () => {
      const editor = view.current;
      if (!editor) {
        return;
      }
      const text = editor.state.doc.toString();
      const insert = text === "" || text.endsWith(" ") ? "" : " ";
      editor.dispatch({ changes: { from: text.length, insert }, selection: { anchor: text.length + insert.length } });
      editor.focus();
      startCompletion(editor);
    },
  }));

  useEffect(() => {
    const content = view.current?.contentDOM;
    if (!content) {
      return;
    }
    const described = [describedBy, problem ? messageId : undefined].filter((id) => id !== undefined).join(" ");
    if (problem) {
      content.setAttribute("aria-invalid", "true");
    } else {
      content.removeAttribute("aria-invalid");
    }
    if (described === "") {
      content.removeAttribute("aria-describedby");
    } else {
      content.setAttribute("aria-describedby", described);
    }
  }, [problem, messageId, describedBy]);

  return (
    <div className={styles.wrap}>
      <div className={classes("field", styles.field, problem !== undefined && styles.invalid)} data-query-field>
        <Filter className="ic" aria-hidden="true" />
        <div ref={host} className={styles.editor} />
        {shortcut}
      </div>
      {problem !== undefined && (
        <p id={messageId} className={styles.problem} role="alert">
          {problem}
        </p>
      )}
    </div>
  );
}
