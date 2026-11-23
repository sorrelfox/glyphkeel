// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, render, screen } from "@testing-library/react";
import { createRef, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18n } from "../i18n/I18n";
import MarkdownEditor from "./MarkdownEditor";
import type { MarkdownEditorHandle, MarkdownEditorProps } from "./types";

afterEach(cleanup);

function renderEditor(props: Partial<MarkdownEditorProps> & { value: string }) {
  const ref = createRef<MarkdownEditorHandle>();
  const utils = render(
    <I18n locale="en-US">
      <MarkdownEditor label="Description" spellChecker={null} ref={ref} {...props} />
    </I18n>,
  );
  return { ...utils, ref };
}

describe("MarkdownEditor", () => {
  it("holds the text exactly and reports every change byte for byte", () => {
    const onChange = vi.fn<(value: string) => void>();
    const text = "# Title\r\n\r\n- [ ] one\r\n";
    const { ref } = renderEditor({ value: text, onChange });
    expect(ref.current?.text()).toBe(text);
    expect(onChange).not.toHaveBeenCalled();
    act(() => {
      ref.current?.insert("a\nb");
    });
    expect(onChange).toHaveBeenLastCalledWith(expect.stringContaining("a\r\nb"));
    expect(ref.current?.text()).toBe(onChange.mock.lastCall?.[0]);
    expect(screen.getByRole("textbox", { name: "Description" })).toBeDefined();
  });

  it("converts mixed line breaks once on opening, says so, and reports the converted text", async () => {
    const onChange = vi.fn<(value: string) => void>();
    const onLineBreaksConverted = vi.fn();
    const { ref } = renderEditor({ value: "a\r\nb\nc\r\n", onChange, onLineBreaksConverted });
    expect(ref.current?.text()).toBe("a\r\nb\r\nc\r\n");
    const note = screen.getByRole("status");
    expect(note.textContent).toContain("CRLF (Windows)");
    expect(onLineBreaksConverted).toHaveBeenCalledWith({ from: { lf: 1, crlf: 2, cr: 0 }, to: "\r\n" });
    await act(() => Promise.resolve());
    expect(onChange).toHaveBeenCalledWith("a\r\nb\r\nc\r\n");
    act(() => {
      screen.getByRole("button", { name: "Dismiss" }).click();
    });
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("takes a new value from outside without echoing it back", () => {
    const onChange = vi.fn<(value: string) => void>();
    let setValue: (value: string) => void = () => undefined;
    const ref = createRef<MarkdownEditorHandle>();
    function Host() {
      const [value, set] = useState("- [ ] one\n- [ ] two\n");
      setValue = set;
      return <MarkdownEditor label="Description" spellChecker={null} value={value} onChange={onChange} ref={ref} />;
    }
    render(
      <I18n locale="en-US">
        <Host />
      </I18n>,
    );
    act(() => {
      setValue("- [ ] one\n- [x] two\n");
    });
    expect(ref.current?.text()).toBe("- [ ] one\n- [x] two\n");
    expect(onChange).not.toHaveBeenCalled();
    act(() => {
      setValue("one\r\ntwo\r\n");
    });
    expect(ref.current?.text()).toBe("one\r\ntwo\r\n");
  });

  it("applies outside edits and refuses those that would split a surrogate pair", () => {
    const { ref } = renderEditor({ value: "a😀b" });
    let applied: boolean | undefined;
    act(() => {
      applied = ref.current?.applyEdits([{ from: 2, to: 2, insert: "x" }]);
    });
    expect(applied).toBe(false);
    act(() => {
      applied = ref.current?.applyEdits([{ from: 3, to: 4, insert: "c" }]);
    });
    expect(applied).toBe(true);
    expect(ref.current?.text()).toBe("a😀c");
  });
});
