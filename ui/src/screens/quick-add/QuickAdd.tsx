// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useMutation } from "@tanstack/react-query";
import { CalendarDays, Flag, Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { useAppearance } from "../../appearance/AppearanceProvider";
import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Select } from "../../components/Select";
import { useProjects } from "../../data/board";
import { errorToast, useCall } from "../../data/hooks";
import { OpFailure } from "../../data/transport";
import { type TaskChange, type TaskCreateInput } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { hideQuickAdd, onQuickAddShown } from "../../ipc/quickAdd";
import { appWindow } from "../../ipc/window";
import { appearanceFrom, readPref, writePref } from "../../prefs/store";
import { parseQuickAdd } from "./parse";
import styles from "./QuickAdd.module.css";

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

  const reset = () => {
    setText("");
    setKeepDate(false);
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

  const submit = () => {
    if (create.isPending) {
      return;
    }
    if (parsed.title === "") {
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
    const base = { project, ...(parsed.priority === undefined ? {} : { priority: parsed.priority }) };
    const dates = parsed.dueText === undefined ? [] : [{ title: parsed.title, dueText: parsed.dueText }, ...(parsed.shorterDates ?? [])];
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
