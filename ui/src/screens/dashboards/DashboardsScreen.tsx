// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { closestCenter, DndContext, PointerSensor, pointerWithin, useSensor, useSensors, type Announcements, type CollisionDetection, type DragEndEvent } from "@dnd-kit/core";
import { rectSortingStrategy, SortableContext } from "@dnd-kit/sortable";
import { useNavigate } from "@tanstack/react-router";
import { MoreHorizontal, Plus } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { useToasts } from "../../components/Toasts";
import { chosenDashboard, clampWidth, moved, useDashboards } from "../../data/dashboards";
import { errorToast, failureText, useCall, useOp, useUndo } from "../../data/hooks";
import { type Dashboard, type DashboardField, type Widget } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { widgetTitle } from "./WidgetBody";
import { WidgetCard } from "./WidgetCard";
import { WidgetDialog, type WidgetDraft } from "./WidgetDialog";
import styles from "./Dashboards.module.css";

/** The widget under the pointer, else the closest one: widgets differ in width, so their centres mislead. */
const underPointer: CollisionDetection = (args) => {
  const within = pointerWithin(args);
  return within.length > 0 ? within : closestCenter(args);
};

/** The list's key for "the whole workspace"; no project key has this form. */
const WORKSPACE = "*";

type Editing = { kind: "new-dashboard" } | { kind: "dashboard"; dashboard: Dashboard } | { kind: "delete"; dashboard: Dashboard } | { kind: "new-widget" } | { kind: "widget"; widget: Widget };

/**
 * Dashboards: the chosen one's widgets in a grid of three columns, each
 * one to three columns wide. Widgets are dragged to another place or
 * moved with the keyboard, made wider or narrower, edited and removed; the
 * choice of dashboard, a menu to make, rename and delete them, and an empty
 * state that makes a starter dashboard.
 */
export function DashboardsScreen({ chosen, from, onChoose }: { chosen: string | undefined; /** The key of the project the page was opened from. */ from?: string | undefined; onChoose: (id: string | undefined) => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const navigate = useNavigate();
  const dashboards = useDashboards();
  const items = dashboards.data?.items ?? [];
  const dashboard = chosenDashboard(items, chosen, from);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [said, setSaid] = useState("");
  // A move shows at once; the engine's answer then takes over.
  const [order, setOrder] = useState<{ dashboard: string; widgets: Widget[] } | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  const close = () => {
    setEditing(null);
    setError(null);
  };
  /** Runs a command: success shows `done` with Undo; a failure stays in the dialog, or shows as a toast. */
  const run = <T,>(work: Promise<T>, done: (output: T) => string | null, inDialog: boolean, after?: (output: T) => void) => {
    work.then(
      (output) => {
        const text = done(output);
        if (text !== null) {
          toasts.show({ tone: "info", text, action: { label: t("toast-undo"), run: undo } });
        }
        if (inDialog) {
          close();
        }
        after?.(output);
      },
      (failure: unknown) => {
        setOrder(null);
        if (inDialog) {
          setError(errorToast(t, failure).text);
        } else {
          toasts.show(errorToast(t, failure));
        }
      },
    );
  };

  const createStarter = (starter: boolean) => {
    run(
      call("dashboard.create", { name: starter ? t("dashboards-starter-name") : t("dashboards-new"), starter }),
      (output) => t("dashboards-created", { name: output.dashboard.name }),
      false,
      (output) => {
        onChoose(output.dashboard.id);
      },
    );
  };

  if (dashboards.isError) {
    return (
      <section className={styles.screen} aria-label={t("view-dashboards")}>
        <p className={styles.problem} role="alert">
          {failureText(t, dashboards.error)}
        </p>
      </section>
    );
  }
  if (!dashboards.data) {
    return <section className={styles.screen} aria-label={t("view-dashboards")} aria-busy="true" />;
  }
  if (dashboard === undefined) {
    return (
      <section className={styles.screen} aria-label={t("view-dashboards")}>
        <div className={styles.empty}>
          <p className="muted">{t("dashboards-empty")}</p>
          <div className={styles.emptyActions}>
            <Button
              className="btn primary"
              onPress={() => {
                createStarter(true);
              }}
            >
              {t("dashboards-create-starter")}
            </Button>
            <Button
              className="btn"
              onPress={() => {
                createStarter(false);
              }}
            >
              {t("dashboards-create-empty")}
            </Button>
          </div>
        </div>
      </section>
    );
  }

  const widgets = order !== null && order.dashboard === dashboard.id ? order.widgets : dashboard.widgets;
  const titleOf = (id: string | number) => {
    const widget = widgets.find((each) => each.id === id);
    return widget === undefined ? "" : widgetTitle(t, widget);
  };
  const placeOf = (id: string | number) => widgets.findIndex((each) => each.id === id) + 1;

  const move = (widget: Widget, place: number) => {
    if (place < 1 || place > widgets.length) {
      return;
    }
    setOrder({ dashboard: dashboard.id, widgets: moved(widgets, widget.id, place) });
    const title = widgetTitle(t, widget);
    setSaid(t("widget-moved", { title, place, count: widgets.length }));
    run(
      call("dashboard.update_widget", { dashboard: dashboard.id, widget: widget.id, position: place }),
      () => null,
      false,
      () => {
        setOrder(null);
      },
    );
  };
  const resize = (widget: Widget, width: number) => {
    const next = clampWidth(width);
    if (next === widget.width) {
      return;
    }
    setSaid(t("widget-width-value", { width: next }));
    run(
      call("dashboard.update_widget", { dashboard: dashboard.id, widget: widget.id, width: next }),
      () => null,
      false,
    );
  };
  const remove = (widget: Widget) => {
    const title = widgetTitle(t, widget);
    run(call("dashboard.remove_widget", { dashboard: dashboard.id, widget: widget.id }), () => t("widget-removed", { title }), false);
  };
  const saveWidget = (draft: WidgetDraft) => {
    if (editing?.kind === "widget") {
      const widget = editing.widget;
      run(
        call("dashboard.update_widget", {
          dashboard: dashboard.id,
          widget: widget.id,
          ...(draft.title !== (widget.title ?? "") ? { title: draft.title } : {}),
          ...(draft.width !== widget.width ? { width: draft.width } : {}),
          settings: draft.settings,
          ...(draft.clear.length > 0 ? { clear: draft.clear } : {}),
        }),
        (output) => t("widget-changed", { title: output.widget === undefined ? draft.title : widgetTitle(t, output.widget) }),
        true,
      );
      return;
    }
    run(
      call("dashboard.add_widget", { dashboard: dashboard.id, widget: { kind: draft.kind, width: draft.width, settings: draft.settings, ...(draft.title === "" ? {} : { title: draft.title }) } }),
      (output) => t("widget-added", { title: output.widget === undefined ? draft.title : widgetTitle(t, output.widget) }),
      true,
    );
  };
  const onDragEnd = (event: DragEndEvent) => {
    const over = event.over;
    if (over === null || over.id === event.active.id) {
      return;
    }
    const widget = widgets.find((each) => each.id === event.active.id);
    if (widget !== undefined) {
      move(widget, placeOf(over.id));
    }
  };
  const announcements: Announcements = {
    onDragStart: ({ active }) => t("widget-drag-start", { title: titleOf(active.id) }),
    onDragOver: ({ over }) => (over === null ? undefined : t("widget-drag-over", { place: placeOf(over.id), count: widgets.length })),
    onDragEnd: ({ active, over }) => t("widget-drag-end", { title: titleOf(active.id), place: over === null ? placeOf(active.id) : placeOf(over.id), count: widgets.length }),
    onDragCancel: ({ active }) => t("widget-drag-cancel", { title: titleOf(active.id) }),
  };
  const openTask = (key: string) => {
    void navigate({ to: "/open/$key", params: { key } });
  };
  const openQuery = (query: string) => {
    void navigate({ to: "/all", search: { query } });
  };

  return (
    <section className={styles.screen} aria-label={t("view-dashboards")}>
      <div className={styles.controls}>
        <Select
          className={styles.choice}
          label={t("dashboards-choose")}
          value={dashboard.id}
          onChange={(id) => {
            setOrder(null);
            onChoose(id);
          }}
          options={items.map((each) => ({ id: each.id, label: each.project === undefined ? each.name : t("dashboards-option-project", { name: each.name, project: each.project }) }))}
        />
        <Button
          className="btn"
          onPress={() => {
            setEditing({ kind: "new-widget" });
          }}
        >
          <Plus className="ic s" aria-hidden="true" />
          <span>{t("dashboards-add-widget")}</span>
        </Button>
        <div className="grow" />
        <MenuTrigger>
          <Button className="btn flat icon" aria-label={t("dashboards-menu")}>
            <MoreHorizontal className="ic" aria-hidden="true" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu
              className="menu"
              onAction={(key) => {
                if (key === "new") setEditing({ kind: "new-dashboard" });
                if (key === "edit") setEditing({ kind: "dashboard", dashboard });
                if (key === "delete") setEditing({ kind: "delete", dashboard });
                if (key === "pin-here" && from !== undefined) {
                  run(call("dashboard.update", { dashboard: dashboard.id, pin_to: from }), (output) => t("dashboards-pinned", { name: output.dashboard.name, project: from }), false);
                }
              }}
            >
              {from !== undefined && dashboard.project !== from && (
                <MenuItem id="pin-here" className="item">
                  {t("dashboards-pin-here", { project: from })}
                </MenuItem>
              )}
              <MenuItem id="new" className="item">
                {t("dashboards-new")}
              </MenuItem>
              <MenuItem id="edit" className="item">
                {t("dashboards-rename")}
              </MenuItem>
              <MenuItem id="delete" className="item">
                {t("dashboards-delete")}
              </MenuItem>
            </Menu>
          </Popover>
        </MenuTrigger>
      </div>
      {dashboard.description !== undefined && <p className="muted">{dashboard.description}</p>}
      {widgets.length === 0 ? (
        <div className={styles.empty}>
          <p className="muted">{t("dashboards-no-widgets")}</p>
          <Button
            className="btn"
            onPress={() => {
              setEditing({ kind: "new-widget" });
            }}
          >
            {t("dashboards-add-widget")}
          </Button>
        </div>
      ) : (
        <DndContext sensors={sensors} collisionDetection={underPointer} onDragEnd={onDragEnd} accessibility={{ announcements, screenReaderInstructions: { draggable: t("widget-move-keys") } }}>
          <SortableContext items={widgets.map((each) => each.id)} strategy={rectSortingStrategy}>
            <div className={styles.grid} role="list" aria-label={t("dashboards-grid")}>
              {widgets.map((widget, index) => (
                  <WidgetCard
                    key={widget.id}
                    dashboard={dashboard.id}
                    widget={widget}
                    place={index + 1}
                    count={widgets.length}
                    onMove={move}
                    onResize={resize}
                    onEdit={(each) => {
                      setEditing({ kind: "widget", widget: each });
                    }}
                    onRemove={remove}
                    onOpenTask={openTask}
                    onOpenQuery={openQuery}
                  />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}
      <p className={styles.srOnly} aria-live="polite">
        {said}
      </p>
      {(editing?.kind === "new-widget" || editing?.kind === "widget") && (
        <WidgetDialog
          dashboard={dashboard}
          {...(editing.kind === "widget" ? { widget: editing.widget } : {})}
          error={error}
          onSave={saveWidget}
          onEdit={() => {
            setError(null);
          }}
          onClose={close}
        />
      )}
      {(editing?.kind === "new-dashboard" || editing?.kind === "dashboard") && (
        <DashboardDialog
          {...(editing.kind === "dashboard" ? { dashboard: editing.dashboard } : {})}
          error={error}
          onEdit={() => {
            setError(null);
          }}
          onClose={close}
          onSave={(input) => {
            if (editing.kind === "dashboard") {
              const old = editing.dashboard;
              const repinned = input.project !== (old.project ?? WORKSPACE);
              const clear: DashboardField[] = [
                ...(input.description === "" && (old.description ?? "") !== "" ? (["description"] as const) : []),
                ...(repinned && input.project === WORKSPACE ? (["pin_to"] as const) : []),
              ];
              run(
                call("dashboard.update", {
                  dashboard: old.id,
                  ...(input.name !== old.name ? { name: input.name } : {}),
                  ...(input.description !== "" && input.description !== old.description ? { description: input.description } : {}),
                  ...(repinned && input.project !== WORKSPACE ? { pin_to: input.project } : {}),
                  ...(clear.length > 0 ? { clear } : {}),
                }),
                (output) => t("dashboards-changed", { name: output.dashboard.name }),
                true,
              );
              return;
            }
            run(
              call("dashboard.create", { name: input.name, starter: input.starter, ...(input.project === WORKSPACE ? {} : { project: input.project }), ...(input.description === "" ? {} : { description: input.description }) }),
              (output) => t("dashboards-created", { name: output.dashboard.name }),
              true,
              (output) => {
                onChoose(output.dashboard.id);
              },
            );
          }}
        />
      )}
      {editing?.kind === "delete" && (
        <Dialog
          title={t("dashboards-delete")}
          isOpen
          onOpenChange={(open) => {
            if (!open) close();
          }}
          footer={
            <>
              <Button className="btn" onPress={close}>
                {t("dialog-cancel")}
              </Button>
              <Button
                className="btn danger"
                onPress={() => {
                  const name = editing.dashboard.name;
                  run(call("dashboard.delete", { dashboard: editing.dashboard.id }), () => t("dashboards-deleted", { name }), true, () => {
                    onChoose(undefined);
                  });
                }}
              >
                {t("dashboards-delete")}
              </Button>
            </>
          }
        >
          <p>{t("dashboards-delete-confirm", { name: editing.dashboard.name })}</p>
          {error !== null && (
            <p className={styles.problem} role="alert">
              {error}
            </p>
          )}
        </Dialog>
      )}
    </section>
  );
}

interface DashboardInput {
  name: string;
  description: string;
  project: string;
  starter: boolean;
}

/** Makes a dashboard (for the whole workspace or pinned to a project, empty or with the starter widgets), or renames one. */
function DashboardDialog({ dashboard, error, onEdit, onSave, onClose }: { dashboard?: Dashboard; error: string | null; onEdit: () => void; onSave: (input: DashboardInput) => void; onClose: () => void }) {
  const t = useT();
  const projects = useOp("project.list", {});
  const [name, setName] = useState(dashboard?.name ?? "");
  const [description, setDescription] = useState(dashboard?.description ?? "");
  const [project, setProject] = useState(dashboard?.project ?? WORKSPACE);
  const [starter, setStarter] = useState(false);
  const save = () => {
    if (name.trim() !== "") {
      onSave({ name: name.trim(), description: description.trim(), project, starter });
    }
  };
  return (
    <Dialog
      title={dashboard === undefined ? t("dashboards-new-title") : t("dashboards-edit-title", { name: dashboard.name })}
      isOpen
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={name.trim() === ""} onPress={save}>
            {dashboard === undefined ? t("dashboards-create") : t("dashboards-save")}
          </Button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
      >
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("dashboards-field-name")}</span>
          <span className="field">
            <input
              autoFocus
              value={name}
              maxLength={100}
              onChange={(event) => {
                onEdit();
                setName(event.target.value);
              }}
            />
          </span>
        </label>
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("dashboards-field-description")}</span>
          <span className="field">
            <input
              value={description}
              maxLength={1000}
              onChange={(event) => {
                onEdit();
                setDescription(event.target.value);
              }}
            />
          </span>
        </label>
        <div className={styles.formRow}>
              <span className={styles.formLabel} aria-hidden="true">
                {t("dashboards-field-project")}
              </span>
              <Select
                label={t("dashboards-field-project")}
                value={project}
                onChange={(next) => {
                  onEdit();
                  setProject(next);
                }}
                options={[{ id: WORKSPACE, label: t("dashboards-whole-workspace") }, ...(projects.data?.items ?? []).map((each) => ({ id: each.key, label: each.path }))]}
              />
            </div>
        {dashboard === undefined && (
          <Switch isSelected={starter} onChange={setStarter}>
            {t("dashboards-field-starter")}
          </Switch>
        )}
        {error !== null && (
          <p className={styles.problem} role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
