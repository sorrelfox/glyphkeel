// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { Button, Menu, MenuTrigger, Popover } from "react-aria-components";
import { afterEach, describe, expect, it } from "vitest";

import { MenuEntries, type MenuEntry } from "./ContextMenu";

afterEach(cleanup);

function Options() {
  const [size, setSize] = useState("regular");
  const [fields, setFields] = useState(["due"]);
  const [ran, setRan] = useState(0);
  const toggle = (id: string) => {
    setFields((was) => (was.includes(id) ? was.filter((other) => other !== id) : [...was, id]));
  };
  const entries: MenuEntry[] = [
    { id: "sizes", section: "Size", items: ["regular", "compact"].map((id) => ({ id, label: id, checked: size === id, run: () => { setSize(id); } })) },
    {
      id: "fields",
      section: "Fields",
      items: [
        ...["due", "labels"].map((id) => ({ id, label: id, checked: fields.includes(id), checkbox: true, run: () => { toggle(id); } })),
        { id: "edit", label: "Edit fields", run: () => { setRan((n) => n + 1); } },
      ],
    },
    "separator",
    { id: "reset", label: "Reset", run: () => { setRan((n) => n + 1); } },
  ];
  return (
    <>
      <output>{`${size} ${fields.join(",")} ${String(ran)}`}</output>
      <MenuTrigger>
        <Button>Options</Button>
        <Popover>
          <Menu aria-label="Options">
            <MenuEntries entries={entries} />
          </Menu>
        </Popover>
      </MenuTrigger>
    </>
  );
}

const open = () => {
  fireEvent.click(screen.getByRole("button", { name: "Options" }));
  return screen.getByRole("menu");
};

describe("menu entries", () => {
  it("are radio and checkbox items that say whether they are chosen, and keep the check mark", () => {
    render(<Options />);
    const menu = open();
    const regular = within(menu).getByRole("menuitemradio", { name: "regular" });
    expect(regular.getAttribute("aria-checked")).toBe("true");
    expect(regular.querySelector(".menuCheck")?.textContent).toBe("✓");
    expect(within(menu).getByRole("menuitemradio", { name: "compact" }).getAttribute("aria-checked")).toBe("false");
    expect(within(menu).getByRole("menuitemcheckbox", { name: "due" }).getAttribute("aria-checked")).toBe("true");
    expect(within(menu).getByRole("menuitemcheckbox", { name: "labels" }).getAttribute("aria-checked")).toBe("false");
    // Plain actions stay menu items, even within a section of choices.
    for (const name of ["Edit fields", "Reset"]) {
      const item = within(menu).getByRole("menuitem", { name });
      expect(item.hasAttribute("aria-checked")).toBe(false);
    }
  });

  it("run their action when chosen and close the menu", () => {
    render(<Options />);
    fireEvent.click(within(open()).getByRole("menuitemradio", { name: "compact" }));
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(within(open()).getByRole("menuitemcheckbox", { name: "labels" }));
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("status").textContent).toBe("compact due,labels 0");
    const menu = open();
    expect(within(menu).getByRole("menuitemradio", { name: "compact" }).getAttribute("aria-checked")).toBe("true");
    expect(within(menu).getByRole("menuitemcheckbox", { name: "labels" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Reset" }));
    expect(screen.getByRole("status").textContent).toBe("compact due,labels 1");
  });
});
