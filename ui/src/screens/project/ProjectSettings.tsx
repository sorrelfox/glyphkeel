// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { useState, type ReactNode } from "react";
import { Button, Tab, TabList, TabPanel, Tabs, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { Select } from "../../components/Select";
import { TypeGlyph } from "../../components/Glyphs";
import { Switch } from "../../components/Switch";
import { useProject } from "../../data/board";
import { GLYPHS, HUES, MODULES, STATUS_CATEGORIES, type Hue, type Project, type Status } from "../../data/catalog";
import { useToasts } from "../../components/Toasts";
import { useCommand, useUndo } from "../../data/hooks";
import { useTemplates } from "../../data/recurrence";
import { type TemplateSummary } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { HuePicker } from "./HuePicker";
import { DeleteQuestion } from "./DeleteProject";
import { FieldsSettings } from "./FieldsSettings";
import { IconPicker } from "./IconPicker";
import { RulesSettings } from "./RulesSettings";
import styles from "./Project.module.css";

interface ProjectSettingsProps {
  projectKey: string;
  onClose: () => void;
}

/**
 * Project settings: general, statuses (with WIP limits), task types,
 * labels and custom fields. Every change applies at once, as in Preferences.
 */
export function ProjectSettings({ projectKey, onClose }: ProjectSettingsProps) {
  const t = useT();
  const project = useProject(projectKey);
  return (
    <Dialog
      title={t("project-settings-title", { name: project.data?.name ?? projectKey })}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) {
          onClose();
        }
      }}
    >
      <div className={styles.settings}>
        {project.data && (
          <Tabs>
            <TabList className="tabs" aria-label={t("project-settings")}>
              <Tab id="general" className={({ isSelected }) => (isSelected ? "tab on" : "tab")}>
                {t("project-settings-general")}
              </Tab>
              <Tab id="statuses" className={({ isSelected }) => (isSelected ? "tab on" : "tab")}>
                {t("project-settings-statuses")}
              </Tab>
              <Tab id="types" className={({ isSelected }) => (isSelected ? "tab on" : "tab")}>
                {t("project-settings-types")}
              </Tab>
              <Tab id="labels" className={({ isSelected }) => (isSelected ? "tab on" : "tab")}>
                {t("project-settings-labels")}
              </Tab>
              <Tab id="fields" className={({ isSelected }) => (isSelected ? "tab on" : "tab")}>
                {t("project-settings-fields")}
              </Tab>
              <Tab id="rules" className={({ isSelected }) => (isSelected ? "tab on" : "tab")}>
                {t("project-settings-rules")}
              </Tab>
              <Tab id="templates" className={({ isSelected }) => (isSelected ? "tab on" : "tab")}>
                {t("project-settings-templates")}
              </Tab>
            </TabList>
            <TabPanel id="general" className={classes(styles.panel)}>
              <General project={project.data} onDeleted={onClose} />
            </TabPanel>
            <TabPanel id="statuses" className={classes(styles.panel)}>
              <Statuses project={project.data} />
            </TabPanel>
            <TabPanel id="types" className={classes(styles.panel)}>
              <Types project={project.data} />
            </TabPanel>
            <TabPanel id="labels" className={classes(styles.panel)}>
              <Labels project={project.data} />
            </TabPanel>
            <TabPanel id="fields" className={classes(styles.panel)}>
              <FieldsSettings project={project.data} />
            </TabPanel>
            <TabPanel id="rules" className={classes(styles.panel)}>
              <RulesSettings project={project.data} />
            </TabPanel>
            <TabPanel id="templates" className={classes(styles.panel)}>
              <Templates project={project.data} />
            </TabPanel>
          </Tabs>
        )}
      </div>
    </Dialog>
  );
}

/** A text field that saves when it loses the focus or on Enter, if it changed. */
function SaveField({ label, value, onSave, className }: { label: string; value: string; onSave: (value: string) => void; className?: string }) {
  const [text, setText] = useState(value);
  const [seen, setSeen] = useState(value);
  if (value !== seen) {
    // The stored value changed (here or elsewhere): show it.
    setSeen(value);
    setText(value);
  }
  const save = () => {
    if (text !== value) {
      onSave(text);
    }
  };
  return (
    <span className={classes("field", className)}>
      <input
        aria-label={label}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            save();
          }
        }}
      />
    </span>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.formRow}>
      <span className={styles.formLabel}>{label}</span>
      {children}
    </div>
  );
}

/**
 * The project's key, and changing it. The old key stays an alias: task
 * keys with it keep working in links, addresses, the command line, agents
 * and queries, and saved queries are rewritten to the new one.
 */
function KeyField({ project }: { project: Project }) {
  const t = useT();
  const { locale } = useLocale();
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const change = useCommand("project.change_key", { undoable: t("toast-project-key-changed") });
  const [editing, setEditing] = useState<string | null>(null);
  const readOnly = project.state === "archived";
  const oldKeys = project.old_keys ?? [];
  const save = () => {
    const key = (editing ?? "").trim().toUpperCase();
    if (key === "" || key === project.key) {
      setEditing(null);
      return;
    }
    const before = project.key;
    change.mutate(
      { project: project.key, key },
      {
        onSuccess: (changed) => {
          setEditing(null);
          // The project's own pages move to the new key; the old address would still work.
          const prefix = `/projects/${before}`;
          if (pathname === prefix || pathname.startsWith(`${prefix}/`)) {
            void navigate({ to: `/projects/${changed.project.key}${pathname.slice(prefix.length)}` });
          }
        },
      },
    );
  };
  if (editing === null) {
    return (
      <div className={styles.keyField}>
        <span className="key" data-testid="project-key">
          {project.key}
        </span>
        <Button className="btn sm" isDisabled={readOnly} onPress={() => { setEditing(project.key); }}>
          {t("project-key-change")}
        </Button>
        {oldKeys.length > 0 && (
          <span className="meta">{t("project-key-old", { keys: new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(oldKeys), count: oldKeys.length })}</span>
        )}
      </div>
    );
  }
  return (
    <form
      className={styles.keyField}
      onSubmit={(event) => {
        event.preventDefault();
        save();
      }}
    >
      <span className="field">
        <input
          autoFocus
          className="num"
          aria-label={t("project-key-new")}
          value={editing}
          maxLength={10}
          onChange={(event) => {
            setEditing(event.target.value.toUpperCase());
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.stopPropagation();
              setEditing(null);
            }
          }}
        />
      </span>
      <Button className="btn primary sm" type="submit" isDisabled={change.isPending || editing.trim() === ""}>
        {t("project-key-save")}
      </Button>
      <Button className="btn sm" onPress={() => { setEditing(null); }}>
        {t("dialog-cancel")}
      </Button>
      <p className="meta">{t("project-key-note", { old: project.key })}</p>
    </form>
  );
}

function General({ project, onDeleted }: { project: Project; onDeleted: () => void }) {
  const t = useT();
  const update = useCommand("project.update", { undoable: t("toast-project-changed") });
  const archive = useCommand("project.archive", { undoable: t("toast-project-archived") });
  const unarchive = useCommand("project.unarchive");
  const patch = (change: Omit<Parameters<typeof update.mutate>[0], "project">) => {
    update.mutate({ project: project.key, ...change });
  };
  return (
    <div className={styles.form}>
      <Row label={t("project-name")}>
        <SaveField label={t("project-name")} value={project.name} onSave={(name) => { patch({ name }); }} />
      </Row>
      <Row label={t("project-key")}>
        <KeyField project={project} />
      </Row>
      <Row label={t("project-colour")}>
        <HuePicker label={t("project-colour")} value={project.hue} onChange={(hue) => { patch({ hue }); }} />
      </Row>
      <Row label={t("project-icon")}>
        <IconPicker
          label={t("project-icon")}
          hue={project.hue}
          value={project.icon}
          onChange={(icon) => {
            patch(icon === undefined ? { clear: ["icon"] } : { icon });
          }}
        />
      </Row>
      <Row label={t("project-description")}>
        <SaveField label={t("project-description")} value={project.description} onSave={(description) => { patch({ description }); }} />
      </Row>
      <Row label={t("project-modules")}>
        <div className={styles.modules}>
          {MODULES.map((module) => (
            <Switch
              key={module}
              isSelected={project.modules[module]}
              onChange={(on) => {
                patch({ modules: { [module]: on } });
              }}
            >
              {t(`module-${module}`)}
            </Switch>
          ))}
        </div>
      </Row>
      <Row label={t("project-state")}>
        {project.state === "active" ? (
          <Button className="btn" onPress={() => { archive.mutate({ project: project.key }); }}>
            {t("project-archive")}
          </Button>
        ) : (
          <Button className="btn" onPress={() => { unarchive.mutate({ project: project.key }); }}>
            {t("project-unarchive")}
          </Button>
        )}
      </Row>
      <Row label={t("project-delete")}>
        <DeleteProject project={project} onDeleted={onDeleted} />
      </Row>
    </div>
  );
}

/** Moves the project and everything under it to the trash, after the question. */
function DeleteProject({ project, onDeleted }: { project: Project; onDeleted: () => void }) {
  const t = useT();
  const [asking, setAsking] = useState(false);
  if (!asking) {
    return (
      <Button className="btn danger" onPress={() => { setAsking(true); }}>
        <Trash2 className="ic s" />
        {t("project-delete-start")}
      </Button>
    );
  }
  return (
    <DeleteQuestion
      project={project}
      onCancel={() => {
        setAsking(false);
      }}
      onDeleted={onDeleted}
    />
  );
}

function HueSelect({ label, value, onChange, className }: { label: string; value: Hue; onChange: (hue: Hue) => void; className?: string | undefined }) {
  const t = useT();
  return (
    <Select
      className={classes(styles.select, className)}
      label={label}
      value={value}
      onChange={onChange}
      options={HUES.map((hue) => ({ id: hue, label: t(`hue-${hue}`), icon: <span className={`dot hue-${hue}`} /> }))}
    />
  );
}

function Statuses({ project }: { project: Project }) {
  const t = useT();
  const update = useCommand("status.update");
  const reorder = useCommand("status.reorder");
  const create = useCommand("status.create");
  const remove = useCommand("status.delete");
  const [deleting, setDeleting] = useState<{ status: Status; target: string } | null>(null);
  const [name, setName] = useState("");
  const ref = { project: project.key };

  return (
    <div className={styles.list}>
      <div className={classes(styles.statusRow, styles.listHead)}>
        <span />
        <span>{t("status-name")}</span>
        <span>{t("status-category")}</span>
        <span>{t("status-colour")}</span>
        <span>{t("status-wip")}</span>
        <span />
      </div>
      {project.statuses.map((status, index) => (
        <div key={status.id} className={styles.statusRow} data-testid={`status-${status.name}`}>
          <span className={`dot hue-${status.hue}`} />
          <SaveField label={t("status-name")} value={status.name} onSave={(value) => { update.mutate({ ...ref, status: status.id, name: value }); }} />
          <Select
            className={classes(styles.select)}
            label={t("status-category")}
            value={status.category}
            onChange={(category) => {
              update.mutate({ ...ref, status: status.id, category });
            }}
            options={STATUS_CATEGORIES.map((category) => ({ id: category, label: t(`category-${category}`) }))}
          />
          <HueSelect label={t("status-colour")} value={status.hue} onChange={(hue) => { update.mutate({ ...ref, status: status.id, hue }); }} />
          <SaveField
            className={classes(styles.narrow)}
            label={t("status-wip")}
            value={status.wip_limit === undefined ? "" : String(status.wip_limit)}
            onSave={(value) => {
              update.mutate(value.trim() === "" ? { ...ref, status: status.id, clear: ["wip_limit"] } : { ...ref, status: status.id, wip_limit: Number(value) });
            }}
          />
          <span className={styles.rowButtons}>
            <Button
              className="btn flat icon sm"
              aria-label={t("status-move-up")}
              isDisabled={index === 0}
              onPress={() => {
                const previous = project.statuses[index - 1];
                if (previous) {
                  reorder.mutate({ ...ref, status: status.id, before: previous.id });
                }
              }}
            >
              <ArrowUp className="ic s" />
            </Button>
            <Button
              className="btn flat icon sm"
              aria-label={t("status-move-down")}
              isDisabled={index === project.statuses.length - 1}
              onPress={() => {
                const next = project.statuses[index + 1];
                if (next) {
                  reorder.mutate({ ...ref, status: status.id, after: next.id });
                }
              }}
            >
              <ArrowDown className="ic s" />
            </Button>
            <Button
              className="btn flat icon sm"
              aria-label={t("status-delete")}
              isDisabled={project.statuses.length < 2}
              onPress={() => {
                const target = project.statuses.find((candidate) => candidate.id !== status.id);
                if (target) {
                  setDeleting({ status, target: target.id });
                }
              }}
            >
              <Trash2 className="ic s" />
            </Button>
          </span>
        </div>
      ))}
      {deleting && (
        <div className={styles.confirm} role="group" aria-label={t("status-delete-title", { name: deleting.status.name })}>
          <span>{t("status-delete-move", { name: deleting.status.name })}</span>
          <Select
            className={classes(styles.select)}
            label={t("status-delete-target")}
            value={deleting.target}
            onChange={(target) => {
              setDeleting({ ...deleting, target });
            }}
            options={project.statuses
              .filter((candidate) => candidate.id !== deleting.status.id)
              .map((candidate) => ({ id: candidate.id, label: candidate.name, icon: <span className={`dot hue-${candidate.hue}`} /> }))}
          />
          <Button className="btn" onPress={() => { setDeleting(null); }}>
            {t("dialog-cancel")}
          </Button>
          <Button
            className="btn danger"
            onPress={() => {
              remove.mutate({ ...ref, status: deleting.status.id, move_tasks_to: deleting.target });
              setDeleting(null);
            }}
          >
            {t("status-delete-confirm", { name: deleting.status.name })}
          </Button>
        </div>
      )}
      <form
        className={styles.addRow}
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim() !== "") {
            create.mutate({ ...ref, name, category: "todo" });
            setName("");
          }
        }}
      >
        <span className="field grow">
          <Plus className="ic" />
          <input aria-label={t("status-add")} placeholder={t("status-add")} value={name} onChange={(event) => { setName(event.target.value); }} />
        </span>
        <button type="submit" className="btn" disabled={name.trim() === ""}>
          {t("settings-add")}
        </button>
      </form>
    </div>
  );
}

function Types({ project }: { project: Project }) {
  const t = useT();
  const update = useCommand("type.update");
  const create = useCommand("type.create");
  const remove = useCommand("type.delete");
  const [name, setName] = useState("");
  const ref = { project: project.key };
  return (
    <div className={styles.list}>
      <div className={classes(styles.simpleRow, styles.listHead)}>
        <span className={styles.typeHeadName}>{t("type-name")}</span>
        <span>{t("type-due-offset-head")}</span>
      </div>
      {project.types.map((type) => (
        <div key={type.id} className={styles.simpleRow}>
          <TypeGlyph glyph={type.glyph} />
          <SaveField className={classes(styles.growField)} label={t("type-name")} value={type.name} onSave={(value) => { update.mutate({ ...ref, type: type.id, name: value }); }} />
          <Select
            className={classes(styles.select)}
            label={t("type-glyph")}
            value={type.glyph}
            onChange={(glyph) => {
              update.mutate({ ...ref, type: type.id, glyph });
            }}
            options={GLYPHS.map((glyph) => ({ id: glyph, label: t(`glyph-${glyph}`), icon: <TypeGlyph glyph={glyph} /> }))}
          />
          <span className={styles.offset}>
            <SaveField
              className={classes(styles.narrow)}
              label={t("type-due-offset", { name: type.name })}
              value={type.due_offset_days === undefined ? "" : String(type.due_offset_days)}
              onSave={(value) => {
                update.mutate(value.trim() === "" ? { ...ref, type: type.id, clear_due_offset: true } : { ...ref, type: type.id, due_offset_days: Math.max(0, Math.trunc(Number(value))) });
              }}
            />
            <span className="muted">{t("type-due-offset-suffix", { count: type.due_offset_days ?? 0 })}</span>
          </span>
          {type.is_default && <span className="chip outline">{t("type-default")}</span>}
          <Button
            className="btn flat icon sm"
            aria-label={t("type-delete", { name: type.name })}
            isDisabled={type.is_default}
            onPress={() => {
              // Its tasks get the default type.
              remove.mutate({ ...ref, type: type.id });
            }}
          >
            <Trash2 className="ic s" />
          </Button>
        </div>
      ))}
      <form
        className={styles.addRow}
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim() !== "") {
            create.mutate({ ...ref, name, glyph: "task" });
            setName("");
          }
        }}
      >
        <span className="field grow">
          <Plus className="ic" />
          <input aria-label={t("type-add")} placeholder={t("type-add")} value={name} onChange={(event) => { setName(event.target.value); }} />
        </span>
        <button type="submit" className="btn" disabled={name.trim() === ""}>
          {t("settings-add")}
        </button>
      </form>
    </div>
  );
}

function Labels({ project }: { project: Project }) {
  const t = useT();
  const update = useCommand("label.update");
  const create = useCommand("label.create");
  const remove = useCommand("label.delete");
  const [name, setName] = useState("");
  // The colour of the next label: the engine's suggestion until one is picked.
  const [hue, setHue] = useState<Hue | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const ref = { project: project.key };
  return (
    <div className={styles.list}>
      {project.labels.length === 0 && <p className="muted">{t("label-none")}</p>}
      {project.labels.map((label) => (
        <div key={label.id} className={styles.simpleRow}>
          <span className={`chip hue-${label.hue}`}>{label.name}</span>
          <SaveField className={classes(styles.growField)} label={t("label-name")} value={label.name} onSave={(value) => { update.mutate({ ...ref, label: label.id, name: value }); }} />
          <HueSelect label={t("label-colour")} value={label.hue} onChange={(hue) => { update.mutate({ ...ref, label: label.id, hue }); }} />
          {confirming === label.id ? (
            <>
              <Button className="btn" onPress={() => { setConfirming(null); }}>
                {t("dialog-cancel")}
              </Button>
              <Button
                className="btn danger"
                onPress={() => {
                  remove.mutate({ ...ref, label: label.id });
                  setConfirming(null);
                }}
              >
                {t("label-delete-confirm", { name: label.name })}
              </Button>
            </>
          ) : (
            <Button className="btn flat icon sm" aria-label={t("label-delete", { name: label.name })} onPress={() => { setConfirming(label.id); }}>
              <Trash2 className="ic s" />
            </Button>
          )}
        </div>
      ))}
      <form
        className={styles.addRow}
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim() !== "") {
            // Without a picked colour the engine gives the suggested one.
            create.mutate(hue === null ? { ...ref, name } : { ...ref, name, hue });
            setName("");
            setHue(null);
          }
        }}
      >
        <span className="field grow">
          <Plus className="ic" />
          <input aria-label={t("label-add")} placeholder={t("label-add")} value={name} onChange={(event) => { setName(event.target.value); }} />
        </span>
        <HueSelect className={classes(styles.addHue)} label={t("label-add-colour")} value={hue ?? project.next_label_hue} onChange={setHue} />
        <button type="submit" className="btn" disabled={name.trim() === ""}>
          {t("settings-add")}
        </button>
      </form>
    </div>
  );
}

/**
 * The project's task templates and the workspace's project templates: their
 * names and descriptions change in place, and they can be deleted (Undo
 * brings one back). The project itself can be saved as a project template.
 */
function Templates({ project }: { project: Project }) {
  const t = useT();
  const own = useTemplates("task", project.key);
  const workspace = useTemplates("project");
  const create = useCommand("template.create");
  const [name, setName] = useState("");
  const [withTasks, setWithTasks] = useState(false);
  return (
    <div className={styles.form}>
      <TemplateList title={t("templates-task")} empty={t("templates-none-task")} items={own.data?.items ?? []} />
      <TemplateList title={t("templates-project")} empty={t("templates-none-project")} items={workspace.data?.items ?? []} />
      <form
        className={styles.templateSave}
        aria-label={t("template-save-project")}
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim() !== "") {
            create.mutate(
              { name: name.trim(), from_project: project.key, include_tasks: withTasks },
              {
                onSuccess: () => {
                  setName("");
                },
              },
            );
          }
        }}
      >
        <h3 className={styles.templateHead}>{t("template-save-project")}</h3>
        <div className={styles.addRow}>
          <span className="field grow">
            <input aria-label={t("template-name")} placeholder={t("template-name")} maxLength={100} value={name} onChange={(event) => { setName(event.target.value); }} />
          </span>
          <button type="submit" className="btn" disabled={name.trim() === "" || create.isPending}>
            {t("template-save-project-button")}
          </button>
        </div>
        <Switch isSelected={withTasks} onChange={setWithTasks}>
          {t("template-include-tasks")}
        </Switch>
      </form>
    </div>
  );
}

function TemplateList({ title, empty, items }: { title: string; empty: string; items: readonly TemplateSummary[] }) {
  const t = useT();
  const update = useCommand("template.update", { undoable: t("toast-template-changed") });
  const remove = useCommand("template.delete");
  const toasts = useToasts();
  const { undo } = useUndo();
  return (
    <section className={styles.list} aria-label={title}>
      <h3 className={styles.templateHead}>{title}</h3>
      {items.length === 0 && <p className="muted">{empty}</p>}
      {items.map((template) => (
        <div key={template.id} className={styles.templateRow} data-template={template.name}>
          <SaveField className={classes(styles.growField)} label={t("template-rename", { name: template.name })} value={template.name} onSave={(value) => { if (value.trim() !== "") { update.mutate({ template: template.id, name: value }); } }} />
          <SaveField className={classes(styles.growField)} label={t("template-describe", { name: template.name })} value={template.description ?? ""} onSave={(value) => { update.mutate({ template: template.id, description: value }); }} />
          <span className="meta">{t(template.kind === "task" ? "template-items-task" : "template-items-project", { count: template.items })}</span>
          <Button className="btn flat icon sm" aria-label={t("template-delete", { name: template.name })} onPress={() => {
              remove.mutate(
                { template: template.id },
                {
                  onSuccess: () => {
                    toasts.show({ tone: "info", text: t("toast-template-deleted", { name: template.name }), action: { label: t("toast-undo"), run: undo } });
                  },
                },
              );
            }}
          >
            <Trash2 className="ic s" />
          </Button>
        </div>
      ))}
    </section>
  );
}
