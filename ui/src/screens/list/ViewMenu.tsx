// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ChevronDown, Star } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Menu, MenuTrigger, Popover } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { Dialog } from "../../components/Dialog";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { errorToast, useCall, useUndo } from "../../data/hooks";
import { OpFailure } from "../../data/transport";
import { useToasts } from "../../components/Toasts";
import { type SavedView } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { type ListSetup } from "./model";
import styles from "./List.module.css";

interface ViewCommandsInput {
  /** The list's project; none for the whole workspace. */
  project: string | undefined;
  views: readonly SavedView[];
  /** The view the list came from; none for all tasks. */
  active: SavedView | undefined;
  /** What the list shows now, saved by Save and Save as. */
  setup: ListSetup;
  onChoose: (view: SavedView | undefined) => void;
  /** After a view was created, renamed or deleted: the one to show (none after a deletion). */
  onSaved: (view: SavedView | undefined) => void;
}

type Editing = { kind: "create" } | { kind: "rename"; view: SavedView } | null;

export interface ViewCommands {
  /** The picker's menu: the views, then Save as, Rename, Make default and Delete. */
  entries: MenuEntry[];
  /** Saves the list into its view. */
  save: () => void;
  /** Opens Save as. */
  saveAs: () => void;
  /** The open dialog, if any. */
  dialog: ReactNode;
}

/**
 * The saved views' commands: choose, save, save as, rename, make default,
 * delete. Each change is one Undo step, offered in its toast.
 */
export function useViewCommands({ project, views, active, setup, onChoose, onSaved }: ViewCommandsInput): ViewCommands {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const [editing, setEditing] = useState<Editing>(null);

  const own = views.filter((view) => project !== undefined && view.project === project);
  const shared = views.filter((view) => view.project === undefined);
  const choose = (view: SavedView): MenuEntry => ({
    id: `view-${view.id}`,
    label: view.is_default ? t("list-view-default-name", { name: view.name }) : view.name,
    checked: active?.id === view.id,
    run: () => {
      onChoose(view);
    },
  });

  const run = async (change: () => Promise<{ view: SavedView; deleted?: boolean }>, done: string): Promise<boolean | string> => {
    try {
      const result = await change();
      onSaved(result.deleted === true ? undefined : result.view);
      toasts.show({ tone: "info", text: done, action: { label: t("toast-undo"), run: undo } });
      return true;
    } catch (error: unknown) {
      if (error instanceof OpFailure && error.error.code !== "ipc") {
        return error.error.message;
      }
      toasts.show(errorToast(t, error));
      return false;
    }
  };
  const runToasting = (change: () => Promise<{ view: SavedView; deleted?: boolean }>, done: string) => {
    void run(change, done).then((result) => {
      if (typeof result === "string") {
        toasts.show({ tone: "error", text: result });
      }
    });
  };

  const save = () => {
    if (!active) {
      return;
    }
    runToasting(
      () =>
        call("view.update", {
          view: active.id,
          query: setup.query,
          columns: [...setup.columns],
          ...(setup.sort === "" ? { clear: ["sort"] } : { sort: setup.sort }),
        }),
      t("list-view-saved", { name: active.name }),
    );
  };

  const entries: MenuEntry[] = [
    {
      id: "all",
      label: t("list-view-none"),
      checked: active === undefined,
      run: () => {
        onChoose(undefined);
      },
    },
    ...(own.length > 0 ? [{ id: "own", section: t("list-views-project"), items: own.map(choose) } as const] : []),
    ...(shared.length > 0 ? [{ id: "shared", section: t("list-views-workspace"), items: shared.map(choose) } as const] : []),
    "separator",
    { id: "save-as", label: t("list-view-save-as"), run: () => { setEditing({ kind: "create" }); } },
    ...(active
      ? ([
          { id: "rename", label: t("list-view-rename"), run: () => { setEditing({ kind: "rename", view: active }); } },
          {
            id: "default",
            label: t(active.is_default ? "list-view-undefault" : "list-view-make-default"),
            run: () => {
              runToasting(() => call("view.update", { view: active.id, is_default: !active.is_default }), t("list-view-saved", { name: active.name }));
            },
          },
          {
            id: "delete",
            label: t("list-view-delete"),
            run: () => {
              runToasting(() => call("view.delete", { view: active.id }), t("list-view-deleted", { name: active.name }));
            },
          },
        ] satisfies MenuEntry[])
      : []),
  ];

  const dialog = editing && (
    <ViewDialog
      editing={editing}
      project={project}
      onClose={() => {
        setEditing(null);
      }}
      onSubmit={(name, forProject, isDefault) =>
        editing.kind === "rename"
          ? run(() => call("view.update", { view: editing.view.id, name }), t("list-view-renamed", { name }))
          : run(
              () =>
                call("view.create", {
                  name,
                  kind: "list",
                  query: setup.query,
                  columns: [...setup.columns],
                  is_default: isDefault,
                  ...(forProject && project !== undefined ? { project } : {}),
                  ...(setup.sort === "" ? {} : { sort: setup.sort }),
                }),
              t("list-view-created", { name }),
            )
      }
    />
  );

  return {
    entries,
    save,
    saveAs: () => {
      setEditing({ kind: "create" });
    },
    dialog,
  };
}

/** The saved view picker: the view's name, a filled star for the default. */
export function ViewPicker({ active, entries }: { active: SavedView | undefined; entries: readonly MenuEntry[] }) {
  const t = useT();
  return (
    <MenuTrigger>
      <Button className={classes("btn", styles.viewButton)} aria-label={t("list-view-button", { name: active?.name ?? t("list-view-none") })}>
        <Star className={classes("ic", active?.is_default === true ? styles.starOn : "muted")} aria-hidden="true" />
        <span className="truncate">{active?.name ?? t("list-view-none")}</span>
        <ChevronDown className="ic s muted" aria-hidden="true" />
      </Button>
      <Popover className="pop menuPopover" placement="bottom start" offset={4}>
        <Menu className="menu" aria-label={t("list-views")}>
          <MenuEntries entries={entries} />
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

/** "Changed since last save", Save (into the view) and Save as. */
export function SaveControls({ changed, active, onSave, onSaveAs }: { changed: boolean; active: SavedView | undefined; onSave: () => void; onSaveAs: () => void }) {
  const t = useT();
  if (!changed) {
    return null;
  }
  return (
    <>
      <span className={classes("meta", styles.changed)} data-testid="list-changed">
        {t("list-view-changed")}
      </span>
      {active && (
        <Button className="btn sm" onPress={onSave}>
          {t("list-view-save")}
        </Button>
      )}
      <Button className={classes("btn sm", active && "flat")} onPress={onSaveAs}>
        {t("list-view-save-as")}
      </Button>
    </>
  );
}

interface ViewDialogProps {
  editing: Exclude<Editing, null>;
  project: string | undefined;
  onClose: () => void;
  /** True when done; the engine's message when it refused. */
  onSubmit: (name: string, forProject: boolean, isDefault: boolean) => Promise<boolean | string>;
}

/** Save as (name, where, default) or Rename (name). */
function ViewDialog({ editing, project, onClose, onSubmit }: ViewDialogProps) {
  const t = useT();
  const [name, setName] = useState(editing.kind === "rename" ? editing.view.name : "");
  const [scope, setScope] = useState<"project" | "workspace">(project === undefined ? "workspace" : "project");
  const [isDefault, setDefault] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = () => {
    if (name.trim() === "") {
      return;
    }
    void onSubmit(name.trim(), scope === "project", isDefault).then((done) => {
      if (done === true) {
        onClose();
      } else if (typeof done === "string") {
        setError(done);
      }
    });
  };
  return (
    <Dialog
      title={t(editing.kind === "rename" ? "list-view-rename-title" : "list-view-save-title")}
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
          <Button className="btn primary" isDisabled={name.trim() === ""} onPress={submit}>
            {t(editing.kind === "rename" ? "list-view-rename-confirm" : "list-view-save-confirm")}
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
          <span className={styles.formLabel}>{t("list-view-name")}</span>
          <span className="field">
            <input
              autoFocus
              value={name}
              onChange={(event) => {
                setName(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        {editing.kind === "create" && project !== undefined && (
          <div className={styles.formRow}>
            <span className={styles.formLabel}>{t("list-view-for")}</span>
            <Segmented
              label={t("list-view-for")}
              segments={[
                { id: "project", label: t("list-view-for-project") },
                { id: "workspace", label: t("list-view-for-workspace") },
              ]}
              value={scope}
              onChange={setScope}
            />
          </div>
        )}
        {editing.kind === "create" && (
          <Switch isSelected={isDefault} onChange={setDefault}>
            {t("list-view-default")}
          </Switch>
        )}
        {error !== null && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
