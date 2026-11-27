// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { AppearanceProvider } from "../../appearance/AppearanceProvider";
import { ToastProvider } from "../../components/Toasts";
import { DataProvider } from "../../data/DataProvider";
import { fakeEngine } from "../../data/testing";
import { I18n } from "../../i18n/I18n";
import { changeEncryption, lockApp, resetFakeLock, updateLock } from "../../ipc/lock";
import { Preferences } from "../Preferences";
import { LockGate } from "./LockGate";
import { LockScreen } from "./LockScreen";

afterEach(cleanup);
afterEach(resetFakeLock);

function setUp(children = <p>Secret task title</p>) {
  const engine = fakeEngine({});
  render(
    <AppearanceProvider onChange={() => undefined}>
      <I18n locale="en-US">
        <ToastProvider>
          <DataProvider client={engine.client}>
            <LockGate>{children}</LockGate>
          </DataProvider>
        </ToastProvider>
      </I18n>
    </AppearanceProvider>,
  );
}

async function relook() {
  // The gate looks at the lock again every few seconds; a test asks at once
  // by locking or unlocking through the screen, or by a fresh render.
  await act(async () => {
    await Promise.resolve();
  });
}

describe("the lock", () => {
  it("shows the app while unlocked", async () => {
    setUp();
    expect(await screen.findByText("Secret task title")).toBeTruthy();
  });

  it("hides everything while locked and unlocks only with the passphrase", async () => {
    await updateLock({ lock_passphrase: "orchard lantern" });
    await lockApp();
    setUp();
    await relook();
    expect(await screen.findByRole("heading", { name: "Glyphkeel is locked" })).toBeTruthy();
    expect(screen.queryByText("Secret task title")).toBeNull();

    const field = screen.getByLabelText("Passphrase");
    fireEvent.change(field, { target: { value: "wrong" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.queryByText("Secret task title")).toBeNull();

    fireEvent.change(screen.getByLabelText("Passphrase"), { target: { value: "orchard lantern" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock" }));
    expect(await screen.findByText("Secret task title")).toBeTruthy();
  });

  it("can't lock without a passphrase to unlock with", async () => {
    await expect(lockApp()).rejects.toThrow();
  });

  it("unlocks an encrypted workspace with the workspace's passphrase", async () => {
    await changeEncryption({ encrypt: true, passphrase: "sea glass" });
    await lockApp();
    setUp();
    await relook();
    fireEvent.change(await screen.findByLabelText("Passphrase"), { target: { value: "sea glass" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock" }));
    expect(await screen.findByText("Secret task title")).toBeTruthy();
  });
});

describe("an encrypted workspace at start", () => {
  it("asks for the workspace's passphrase to open it", async () => {
    setUp(<LockScreen view={{ locked: true, reason: "workspace", can_lock: true, lock_passphrase: false, encryption: { available: true, encrypted: true } }} />);
    expect(await screen.findByRole("heading", { name: "This workspace is encrypted" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open" })).toBeTruthy();
    expect(screen.getByLabelText("Passphrase")).toBeTruthy();
  });
});

describe("Preferences, Security", () => {
  it("sets a lock passphrase, then offers the idle lock and locking now", async () => {
    setUp(<Preferences isOpen onOpenChange={() => undefined} page="security" />);
    const lockNow = await screen.findByRole<HTMLButtonElement>("button", { name: "Lock" });
    expect(lockNow.disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Set…" }));
    fireEvent.change(screen.getByLabelText("New passphrase"), { target: { value: "orchard lantern" } });
    fireEvent.change(screen.getByLabelText("The same passphrase again"), { target: { value: "orchard lanter" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect((await screen.findByRole("alert")).textContent).toContain("differ");
    fireEvent.change(screen.getByLabelText("The same passphrase again"), { target: { value: "orchard lantern" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(screen.getByRole<HTMLButtonElement>("button", { name: "Lock" }).disabled).toBe(false);
    });
    expect(screen.getByText("A passphrase is set.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Lock" }));
    expect(await screen.findByRole("heading", { name: "Glyphkeel is locked" })).toBeTruthy();
  });

  it("encrypts the workspace with a passphrase given twice", async () => {
    setUp(<Preferences isOpen onOpenChange={() => undefined} page="security" />);
    fireEvent.click(await screen.findByRole("button", { name: "Encrypt…" }));
    fireEvent.change(screen.getByLabelText("New passphrase"), { target: { value: "sea glass" } });
    fireEvent.change(screen.getByLabelText("The same passphrase again"), { target: { value: "sea glass" } });
    fireEvent.click(screen.getByRole("button", { name: "Encrypt" }));
    expect(await screen.findByText("The workspace is encrypted.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Change passphrase…" })).toBeTruthy();
  });
});
