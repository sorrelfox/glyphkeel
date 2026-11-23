// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Tab, TabList, TabPanel, Tabs } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { TypeGlyph } from "../../components/Glyphs";
import { Switch } from "../../components/Switch";
import { useProject } from "../../data/board";
import { GLYPHS, HUES, MODULES, STATUS_CATEGORIES, type Hue, type Project, type Status } from "../../data/catalog";
import { useCommand } from "../../data/hooks";
import { useT } from "../../i18n/I18n";
import { HuePicker } from "./HuePicker";
import styles from "./Project.module.css";

interface ProjectSettingsProps {
  projectKey: string;
  onClose: () => void;
}

/**
 * Project settings: general, statuses (with WIP limits), task types and
 * labels. Every change applies at once, as in Preferences.
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
            </TabList>
            <TabPanel id="general" className={classes(styles.panel)}>
              <General project={project.data} />
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

function General({ project }: { project: Project }) {
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
        <span className="key">{project.key}</span>
      </Row>
      <Row label={t("project-colour")}>
        <HuePicker label={t("project-colour")} value={project.hue} onChange={(hue) => { patch({ hue }); }} />
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
    </div>
  );
}

function HueSelect({ label, value, onChange }: { label: string; value: Hue; onChange: (hue: Hue) => void }) {
  const t = useT();
  return (
    <span className={classes("field", styles.select)}>
      <span className={`dot hue-${value}`} />
      <select
        aria-label={label}
        value={value}
        onChange={(event) => {
          const hue = HUES.find((candidate) => candidate === event.target.value);
          if (hue) {
            onChange(hue);
          }
        }}
      >
        {HUES.map((hue) => (
          <option key={hue} value={hue}>
            {t(`hue-${hue}`)}
          </option>
        ))}
      </select>
    </span>
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
          <span className={classes("field", styles.select)}>
            <select
              aria-label={t("status-category")}
              value={status.category}
              onChange={(event) => {
                const category = STATUS_CATEGORIES.find((candidate) => candidate === event.target.value);
                if (category) {
                  update.mutate({ ...ref, status: status.id, category });
                }
              }}
            >
              {STATUS_CATEGORIES.map((category) => (
                <option key={category} value={category}>
                  {t(`category-${category}`)}
                </option>
              ))}
            </select>
          </span>
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
          <span className={classes("field", styles.select)}>
            <select
              aria-label={t("status-delete-target")}
              value={deleting.target}
              onChange={(event) => {
                setDeleting({ ...deleting, target: event.target.value });
              }}
            >
              {project.statuses
                .filter((candidate) => candidate.id !== deleting.status.id)
                .map((candidate) => (
                  <option key={candidate.id} value={candidate.id}>
                    {candidate.name}
                  </option>
                ))}
            </select>
          </span>
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
      {project.types.map((type) => (
        <div key={type.id} className={styles.simpleRow}>
          <TypeGlyph glyph={type.glyph} />
          <SaveField className={classes(styles.growField)} label={t("type-name")} value={type.name} onSave={(value) => { update.mutate({ ...ref, type: type.id, name: value }); }} />
          <span className={classes("field", styles.select)}>
            <select
              aria-label={t("type-glyph")}
              value={type.glyph}
              onChange={(event) => {
                const glyph = GLYPHS.find((candidate) => candidate === event.target.value);
                if (glyph) {
                  update.mutate({ ...ref, type: type.id, glyph });
                }
              }}
            >
              {GLYPHS.map((glyph) => (
                <option key={glyph} value={glyph}>
                  {t(`glyph-${glyph}`)}
                </option>
              ))}
            </select>
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
            create.mutate({ ...ref, name });
            setName("");
          }
        }}
      >
        <span className="field grow">
          <Plus className="ic" />
          <input aria-label={t("label-add")} placeholder={t("label-add")} value={name} onChange={(event) => { setName(event.target.value); }} />
        </span>
        <button type="submit" className="btn" disabled={name.trim() === ""}>
          {t("settings-add")}
        </button>
      </form>
    </div>
  );
}
