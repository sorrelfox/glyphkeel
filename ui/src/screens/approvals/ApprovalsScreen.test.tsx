// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type ProposalList } from "../../generated/ops";

// jsdom lays nothing out; the editor measures text ranges.
Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
Range.prototype.getBoundingClientRect = () => new DOMRect();

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

const waiting = async (transport: ReturnType<typeof fakeTransport>) => ((await transport.call("proposal.list", { status: "pending" })) as ProposalList).pending;

describe("the Approvals screen", () => {
  it("lists waiting changes by session with their diff, and approves and rejects them", async () => {
    const transport = fakeTransport({ sampleProposals: true });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/approvals" />);
    const page = await screen.findByRole("region", { name: "Approvals" });
    const build = await within(page).findByRole("region", { name: (name) => plain(name).startsWith("Build agent · session") });
    const cards = within(build).getAllByRole("listitem", { name: (name) => name !== "" });
    expect(cards.length).toBeGreaterThanOrEqual(3);
    const rename = cards.find((card) => card.querySelector("[data-testid=proposal-conflicts]") !== null);
    expect(rename).toBeDefined();
    expect(within(rename as HTMLElement).getByRole("button", { name: "Approve anyway" })).toBeDefined();
    expect(within(build).getAllByTestId("proposal-diff").some((diff) => plain(diff.textContent).includes("Priority:"))).toBe(true);

    // The sidebar counts what waits.
    const nav = screen.getByRole("button", { name: (name) => plain(name).startsWith("Approvals") });
    await waitFor(() => {
      expect(plain(nav.textContent)).toContain("4");
    });

    // An interrupted approval can only be set aside; a failed one says why.
    const docs = within(page).getByRole("region", { name: (name) => plain(name).startsWith("Docs agent") });
    expect(within(docs).getByTestId("proposal-interrupted")).toBeDefined();
    expect(within(docs).queryByRole("button", { name: "Approve" })).toBeNull();
    expect(within(docs).getByRole("button", { name: "Set aside…" })).toBeDefined();
    expect(within(page).getByTestId("proposal-failed")).toBeDefined();

    const comment = cards.find((card) => plain(card.getAttribute("aria-label")) === "Add a comment") as HTMLElement;
    fireEvent.click(within(comment).getByRole("button", { name: "Approve" }));
    await waitFor(async () => {
      expect(await waiting(transport)).toBe(3);
    });

    const review = within(page).getByRole("region", { name: (name) => plain(name).startsWith("Review agent") });
    fireEvent.click(within(review).getByRole("button", { name: "Reject…" }));
    const dialog = await screen.findByRole("dialog", { name: "Reject the change" });
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "Keep it." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Reject" }));
    await waitFor(async () => {
      expect(await waiting(transport)).toBe(2);
    });
    const rejected = (await transport.call("proposal.list", { status: "rejected" })) as ProposalList;
    expect(rejected.items[0]?.reason).toBe("Keep it.");
  });

  it("says when nothing waits", async () => {
    const transport = fakeTransport();
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/approvals" />);
    expect(await screen.findByTestId("approvals-none")).toBeDefined();
  });
});
