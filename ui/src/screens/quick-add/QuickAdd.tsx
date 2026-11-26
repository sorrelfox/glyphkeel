// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useMutation } from "@tanstack/react-query";
import { CalendarDays, Flag, Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useLocale } from "react-aria-components";

import { useAppearance } from "../../appearance/AppearanceProvider";
import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Select } from "../../components/Select";
import { useProjects } from "../../data/board";
import { errorToast, useCall } from "../../data/hooks";
import { useTemplates } from "../../data/recurrence";
import { OpFailure } from "../../data/transport";
import { type TaskChange, type TaskCreateInput, type TemplateApplied, type TemplateApplyInput } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { hideQuickAdd, onQuickAddShown } from "../../ipc/quickAdd";
import { appWindow } from "../../ipc/window";
import { appearanceFrom, readPref, writePref } from "../../prefs/store";
import { parseQuickAdd } from "./parse";
import styles from "./QuickAdd.module.css";

/** No template: the template choice's first entry. */
const NO_TEMPLATE = "none";

/** The project new tasks went to last, kept for the next time. */
const LAST_PROJECT = "quickAdd.project";

/** Whether the engine refused a task because it could not read its date. */
function unreadDate(error: unknown): boolean {
  return error instanceof OpFailure && error.error.code !== "ipc" && error.error.details?.message_id === "date-invalid";
}

/**
 * The quick-add window: one line for a task's title, with the project, a due
 * date and a priority typed inline or picked below. Enter adds the task and
 * closes the window; Escape closes it. The desktop gives the focus back to
 * the window that had it.
 */
export function QuickAdd() {
  const t = useT();
  const call = useCall();
  const projects = useProjects();
  const { appearance, update } = useAppearance();
  const { locale } = useLocale();
  const input = useRef<HTMLInputElement>(null);
  const [text, setText] = useState("");
  const [keepDate, setKeepDate] = useState(false);
  const [picked, setPicked] = useState<string | null>(() => {
    const stored = readPref(LAST_PROJECT);
    return typeof stored === "string" ? stored : null;
  });
  const [note, setNote] = useState<{ tone: "info" | "error"; text: string } | null>(null);

  const active = (projects.data?.items ?? []).filter((project) => project.state === "active");
  const choices = active.map((project) => ({ key: project.key, name: project.name }));
  const parsed = parseQuickAdd(text, { projects: choices, keepDate });
  const fallback = active.find((project) => project.key === picked) ?? active[0];
  const project = parsed.project ?? fallback?.key;
  // A task template of the chosen project to start from; another project's is none.
  const [pickedTemplate, setTemplate] = useState<{ project: string; id: string } | null>(null);
  const templates = useTemplates("task", project ?? "", project !== undefined);
  const template = pickedTemplate !== null && pickedTemplate.project === project && (templates.data?.items ?? []).some((each) => each.id === pickedTemplate.id) ? pickedTemplate.id : null;

  const reset = () => {
    setText("");
    setKeepDate(false);
    setTemplate(null);
    input.current?.focus();
  };

  useEffect(() => {
    void appWindow.setTitle(t("quick-add-title"));
  }, [t]);

  // Each time the window shows: an empty field with the focus, in the
  // appearance the main window has now.
  const onShown = useRef<(stored: unknown) => void>(() => undefined);
  useEffect(() => {
    onShown.current = (stored) => {
      setText("");
      setKeepDate(false);
      setNote(null);
      const next = appearanceFrom(stored);
      if (JSON.stringify(next) !== JSON.stringify(appearance)) {
        update(next);
      }
      input.current?.focus();
    };
  });
  useEffect(
    () =>
      onQuickAddShown((stored) => {
        onShown.current(stored);
      }),
    [],
  );
  useEffect(() => {
    input.current?.focus();
  }, []);

  // The engine reads the date. Should it not read the words at the end, a
  // shorter date is tried; with none left, its answer about the first.
  const create = useMutation({
    mutationFn: async (tries: TaskCreateInput[]): Promise<TaskChange> => {
      let first: unknown = null;
      for (const task of tries) {
        try {
          return await call("task.create", task);
        } catch (error) {
          first ??= error;
          if (!unreadDate(error)) {
            throw error;
          }
        }
      }
      throw first;
    },
    onSuccess: async (change) => {
      setNote({ tone: "info", text: t("quick-add-added", { key: change.task.key }) });
      reset();
      await hideQuickAdd();
    },
    onError: (error) => {
      setNote({ tone: "error", text: errorToast(t, error).text });
    },
  });

  // As for a task: should the engine not read the date, a shorter one is tried.
  const fromTemplate = useMutation({
    mutationFn: async (tries: TemplateApplyInput[]): Promise<TemplateApplied> => {
      let first: unknown = null;
      for (const each of tries) {
        try {
          return await call("template.apply", each);
        } catch (error) {
          first ??= error;
          if (!unreadDate(error)) {
            throw error;
          }
        }
      }
      throw first;
    },
    onSuccess: async (applied) => {
      const leftOut = applied.left_out ?? [];
      setNote({
        tone: "info",
        text: leftOut.length > 0 ? t("template-left-out", { count: leftOut.length, names: new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(leftOut) }) : t("quick-add-added", { key: applied.task?.key ?? "" }),
      });
      reset();
      if (leftOut.length === 0) {
        await hideQuickAdd();
      }
    },
    onError: (error) => {
      setNote({ tone: "error", text: errorToast(t, error).text });
    },
  });

  const submit = () => {
    if (create.isPending || fromTemplate.isPending) {
      return;
    }
    if (parsed.title === "" && template === null) {
      setNote({ tone: "error", text: t("quick-add-need-title") });
      return;
    }
    if (project === undefined) {
      setNote({ tone: "error", text: t("quick-add-no-projects") });
      return;
    }
    setNote(null);
    if (parsed.project === undefined) {
      writePref(LAST_PROJECT, project);
    }
    const dates = parsed.dueText === undefined ? [] : [{ title: parsed.title, dueText: parsed.dueText }, ...(parsed.shorterDates ?? [])];
    if (template !== null) {
      // The due date and priority typed inline win over the template's.
      const base = { template, project, ...(parsed.priority === undefined ? {} : { priority: parsed.priority }) };
      const titled = (title: string) => (title === "" ? {} : { title });
      fromTemplate.mutate(dates.length === 0 ? [{ ...base, ...titled(parsed.title) }] : dates.map((date) => ({ ...base, ...titled(date.title), due_date: date.dueText })));
      return;
    }
    const base = { project, ...(parsed.priority === undefined ? {} : { priority: parsed.priority }) };
    create.mutate(dates.length === 0 ? [{ ...base, title: parsed.title }] : dates.map((date) => ({ ...base, title: date.title, due_date: date.dueText })));
  };

  const close = () => {
    reset();
    setNote(null);
    void hideQuickAdd();
  };

  return (
    <div
      className={styles.window}
      role="dialog"
      aria-label={t("quick-add-title")}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented) {
          event.preventDefault();
          close();
        }
      }}
    >
      <label className={classes("field", styles.field)}>
        <Plus className="ic" aria-hidden="true" />
        <input
          ref={input}
          type="text"
          aria-label={t("quick-add-field")}
          placeholder={t("quick-add-placeholder")}
          value={text}
          spellCheck={false}
          onChange={(event) => {
            setText(event.target.value);
            if (event.target.value === "") {
              setKeepDate(false);
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent.isComposing) {
              event.preventDefault();
              submit();
            }
          }}
        />
      </label>
      <div className={styles.options}>
        {active.length > 0 && project !== undefined && (
          <Select
            label={t("quick-add-project")}
            className={styles.project}
            isDisabled={parsed.project !== undefined}
            value={project}
            options={active.map((each) => ({ id: each.key, label: each.path, icon: <ProjectMark hue={each.hue} icon={each.icon} /> }))}
            onChange={(key) => {
              setPicked(key);
              writePref(LAST_PROJECT, key);
              input.current?.focus();
            }}
          />
        )}
        {(templates.data?.items.length ?? 0) > 0 && (
          <Select
            label={t("template-picker")}
            className={styles.project}
            value={template ?? NO_TEMPLATE}
            options={[{ id: NO_TEMPLATE, label: t("template-none") }, ...(templates.data?.items ?? []).map((each) => ({ id: each.id, label: each.name }))]}
            onChange={(id) => {
              setTemplate(id === NO_TEMPLATE || project === undefined ? null : { project, id });
              input.current?.focus();
            }}
          />
        )}
        {parsed.dueText !== undefined && (
          <span className={classes("chip outline", styles.chip)} data-testid="quick-add-due">
            <CalendarDays className="ic s" aria-hidden="true" />
            {t("quick-add-due", { words: parsed.dueText })}
            <button
              type="button"
              className={styles.drop}
              aria-label={t("quick-add-keep-date", { words: parsed.dueText })}
              onClick={() => {
                setKeepDate(true);
                input.current?.focus();
              }}
            >
              <X className="ic s" aria-hidden="true" />
            </button>
          </span>
        )}
        {parsed.priority !== undefined && (
          <span className={classes("chip outline", styles.chip)} data-testid="quick-add-priority">
            <Flag className="ic s" aria-hidden="true" />
            {t(`priority-${parsed.priority}`)}
          </span>
        )}
        {parsed.unknownProject !== undefined && <span className={classes("meta", styles.warning)}>{t("quick-add-unknown-project", { token: parsed.unknownProject })}</span>}
        <span className={styles.hints}>
          <span className={styles.hint}>
            <span className="kbd">{t("key-enter")}</span>
            {t("quick-add-hint-add")}
          </span>
          <span className={styles.hint}>
            <span className="kbd">{t("key-escape")}</span>
            {t("quick-add-hint-close")}
          </span>
        </span>
      </div>
      <p className={classes(styles.note, note?.tone === "error" && styles.error)} role={note?.tone === "error" ? "alert" : "status"}>
        {note?.text ?? (active.length === 0 && projects.isSuccess ? t("quick-add-no-projects") : t("quick-add-syntax"))}
      </p>
    </div>
  );
}
