// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Check, HardDrive } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuItem, MenuSection, MenuTrigger, Popover, Separator } from "react-aria-components";

import { Dialog } from "../components/Dialog";
import { errorToast } from "../data/hooks";
import { useT } from "../i18n/I18n";
import { useChangeWorkspace, useWorkspaces, type WorkspacesView } from "../ipc/workspaces";
import styles from "../screens/project/Project.module.css";

const NEW = "new";
const RENAME = "rename";

/**
 * The workspace's name in the status bar, as a menu to switch: the
 * workspaces of the data folder (the open one ticked), a new one, and
 * renaming the open one. Where the page can't switch (a plain browser), the
 * name alone.
 */
export function WorkspaceSwitcher({ name, title }: { name: string; title?: string | undefined }) {
  const t = useT();
  const workspaces = useWorkspaces();
  const change = useChangeWorkspace();
  const [dialog, setDialog] = useState<"new" | "rename" | null>(null);
  const view = workspaces.data;
  if (!view) {
    return (
      <span className="meta" data-testid="status-workspace" title={title}>
        <HardDrive className="ic" />
        {name}
      </span>
    );
  }
  const current = view.current;
  return (
    <>
      <MenuTrigger>
        <Button className="btn flat sm meta" data-testid="status-workspace" aria-label={t("workspace-menu", { name })}>
          <HardDrive className="ic" />
          <span className="truncate">{name}</span>
        </Button>
        <Popover className="pop menuPopover" placement="top end" offset={4}>
          <Menu
            className="menu"
            aria-label={t("workspace-menu", { name })}
            onAction={(id) => {
              if (id === NEW) {
                setDialog("new");
              } else if (id === RENAME) {
                setDialog("rename");
              } else {
                change.mutate({ kind: "open", name: String(id).slice("open:".length) });
              }
            }}
          >
            {/* The workspaces are a choice: the open one is checked (aria-checked). */}
            <MenuSection selectionMode="single" selectedKeys={view.items.filter((item) => item.current).map((item) => `open:${item.name}`)}>
              {view.items.map((item) => (
                <MenuItem key={item.name} id={`open:${item.name}`} className="item" textValue={item.name} data-testid={`workspace-${item.name}`}>
                  <Check className="ic s" aria-hidden="true" style={{ visibility: item.current ? "visible" : "hidden" }} />
                  <span className="grow truncate">{item.name}</span>
                </MenuItem>
              ))}
            </MenuSection>
            {view.items.length > 0 && <Separator className="menuSeparator" />}
            <MenuItem id={NEW} className="item" textValue={t("workspace-new")}>
              <span className="grow truncate">{t("workspace-new")}</span>
            </MenuItem>
            <MenuItem id={RENAME} className="item" textValue={t("workspace-rename")} isDisabled={current === undefined || view.elsewhere}>
              <span className="grow truncate">{t("workspace-rename")}</span>
            </MenuItem>
          </Menu>
        </Popover>
      </MenuTrigger>
      {dialog && (
        <WorkspaceNameDialog
          mode={dialog}
          view={view}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
    </>
  );
}

/** Asks for the name of a new workspace, or the open one's new name. */
function WorkspaceNameDialog({ mode, view, onClose }: { mode: "new" | "rename"; view: WorkspacesView; onClose: () => void }) {
  const t = useT();
  const change = useChangeWorkspace({ quiet: true });
  const old = view.current ?? "";
  const [name, setName] = useState(mode === "rename" ? old : "");
  const [error, setError] = useState<string | null>(null);
  const submit = () => {
    const wanted = name.trim();
    if (wanted === "") {
      return;
    }
    change.mutate(mode === "new" ? { kind: "create", name: wanted } : { kind: "rename", old, name: wanted }, {
      onSuccess: onClose,
      onError: (failure) => {
        const shown = errorToast(t, failure);
        setError([shown.text, shown.detail].filter(Boolean).join(" "));
      },
    });
  };
  return (
    <Dialog
      title={mode === "new" ? t("workspace-new-title") : t("workspace-rename-title", { name: old })}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={name.trim() === "" || change.isPending} onPress={submit}>
            {mode === "new" ? t("workspace-create") : t("workspace-rename-confirm")}
          </Button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("workspace-name")}</span>
          <span className="field">
            <input
              autoFocus
              value={name}
              maxLength={64}
              data-testid="workspace-name"
              onChange={(event) => {
                setName(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        <p className="meta">{mode === "new" ? t("workspace-new-note") : t("workspace-rename-note")}</p>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
