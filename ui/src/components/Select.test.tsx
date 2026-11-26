// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18n } from "../i18n/I18n";
import { Select } from "./Select";

afterEach(cleanup);

const FRUIT = [
  { id: "apple", label: "Apple" },
  { id: "banana", label: "Banana" },
  { id: "cherry", label: "Cherry", icon: <span data-testid="cherry-dot" /> },
] as const;
type Fruit = (typeof FRUIT)[number]["id"];

function Fruits({ onChange }: { onChange?: (fruit: Fruit) => void }) {
  const [value, setValue] = useState<Fruit>("banana");
  return (
    <I18n locale="en-US">
      <Select
        label="Fruit"
        options={FRUIT}
        value={value}
        onChange={(fruit) => {
          setValue(fruit);
          onChange?.(fruit);
        }}
      />
    </I18n>
  );
}

function trigger() {
  return screen.getByRole("button", { name: /Fruit/ });
}

describe("Select", () => {
  it("renders no native select, and shows the chosen value with its name", () => {
    render(<Fruits />);
    expect(document.querySelector("select:not([aria-hidden] select, [aria-hidden=true] select)")).toBeNull();
    expect(trigger().textContent).toContain("Banana");
    expect(trigger().getAttribute("aria-haspopup")).toBe("listbox");
    // The name holds the text on screen, after the field's own name.
    expect(trigger().getAttribute("aria-label")).toBe("Fruit: Banana");
  });

  it("opens the design's popover with every option and a check at the chosen one", async () => {
    render(<Fruits />);
    const field = trigger();
    fireEvent.click(field);
    const list = await screen.findByRole("listbox");
    const options = within(list).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["Apple", "Banana", "Cherry"]);
    expect(options.map((option) => option.getAttribute("aria-selected"))).toEqual(["false", "true", "false"]);
    expect(list.closest(".pop")).not.toBeNull();
    expect(options.every((option) => option.classList.contains("item"))).toBe(true);
    // The popover lives in a portal on the body, under <html> where the theme is set.
    expect(list.closest("body > *")).not.toBe(field.closest("body > *"));
    expect(within(list).getByTestId("cherry-dot")).toBeDefined();
  });

  it("chooses an option with the pointer and closes", async () => {
    const onChange = vi.fn();
    render(<Fruits onChange={onChange} />);
    fireEvent.click(trigger());
    fireEvent.click(within(await screen.findByRole("listbox")).getByRole("option", { name: "Cherry" }));
    expect(onChange).toHaveBeenCalledWith("cherry");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(trigger().textContent).toContain("Cherry");
    // The chosen option's icon shows in the field too.
    expect(within(trigger()).getByTestId("cherry-dot")).toBeDefined();
  });

  it("works from the keyboard: arrows open and move, Enter chooses", async () => {
    const onChange = vi.fn();
    render(<Fruits onChange={onChange} />);
    act(() => {
      trigger().focus();
    });
    fireEvent.keyDown(trigger(), { key: "ArrowDown" });
    const list = await screen.findByRole("listbox");
    const focused = () => list.querySelector("[data-focused]")?.textContent;
    expect(focused()).toBe("Banana");
    fireEvent.keyDown(document.activeElement ?? list, { key: "ArrowDown" });
    expect(focused()).toBe("Cherry");
    fireEvent.keyDown(document.activeElement ?? list, { key: "Enter" });
    fireEvent.keyUp(document.activeElement ?? list, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("cherry");
  });

  it("jumps to an option by typing its first letters", () => {
    const onChange = vi.fn();
    render(<Fruits onChange={onChange} />);
    act(() => {
      trigger().focus();
    });
    // On the closed field, typeahead chooses directly, as a native select does.
    fireEvent.keyDown(trigger(), { key: "a" });
    expect(onChange).toHaveBeenCalledWith("apple");
  });
});
