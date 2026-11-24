// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { Button } from "react-aria-components";

import { DENSITIES } from "../appearance/appearance";
import { useAppearance } from "../appearance/AppearanceProvider";
import { classes } from "../classes";
import { Dialog } from "../components/Dialog";
import { Segmented } from "../components/Segmented";
import { useCall, useOp } from "../data/hooks";
import { OpFailure } from "../data/transport";
import { useT } from "../i18n/I18n";
import { useAppSettings } from "../ipc/appSettings";
import { readPref, writePref } from "../prefs/store";
import { suggestKey } from "../screens/project/NewProjectDialog";
import { type StartData } from "../start";
import styles from "./FirstRun.module.css";
import { ThemeTiles } from "./Preferences";

const STEPS = ["workspace", "look", "project", "agents"] as const;
type Step = (typeof STEPS)[number];

/** Where the set-up stands, kept with this person's preferences. */
const PREF = "firstRun";

/**
 * Whether to offer the set-up: the app made the workspace at this start,
 * or the set-up was offered before and not finished or skipped.
 */
export function offersFirstRun(start: Pick<StartData, "firstRun">): boolean {
  const state = readPref(PREF);
  if (state === "done") {
    return false;
  }
  return start.firstRun === true || state === "pending";
}

/** Remembers that the set-up was offered, so it comes back if the app closes before it ends. */
export function markFirstRunPending(): void {
  if (readPref(PREF) !== "done") {
    writePref(PREF, "pending");
  }
}

interface FirstRunProps {
  /** Ends the set-up, finished or skipped; it isn't offered again. */
  onClose: () => void;
  /** Ends it and opens Preferences at Agents & CLI. */
  onSetUpAgents: () => void;
}

/**
 * The first-run set-up, in four short steps: the workspace and where it
 * lives, the look, an optional first project, and agents. Every step can
 * be skipped, and Esc skips the rest. It works with the keyboard alone: the
 * main button of each step has the focus.
 */
export function FirstRun({ onClose, onSetUpAgents }: FirstRunProps) {
  const t = useT();
  const [step, setStep] = useState<Step>("workspace");
  const index = STEPS.indexOf(step);
  const finish = () => {
    writePref(PREF, "done");
    onClose();
  };
  const go = (by: number) => {
    const next = STEPS[index + by];
    if (next) {
      setStep(next);
    }
  };
  const last = index === STEPS.length - 1;
  return (
    <Dialog
      title={t("first-run-title")}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          finish();
        }
      }}
      footer={
        <>
          <Button className="btn flat" onPress={finish}>
            {t("first-run-skip")}
          </Button>
          <span className={classes("muted num", styles.progress)}>{t("first-run-step", { step: index + 1, steps: STEPS.length })}</span>
          {index > 0 && (
            <Button
              className="btn"
              onPress={() => {
                go(-1);
              }}
            >
              {t("first-run-back")}
            </Button>
          )}
          {last ? (
            <>
              <Button
                className="btn"
                onPress={() => {
                  writePref(PREF, "done");
                  onSetUpAgents();
                }}
              >
                {t("first-run-agents-open")}
              </Button>
              <Button key="finish" className="btn primary" autoFocus onPress={finish}>
                {t("first-run-finish")}
              </Button>
            </>
          ) : (
            <Button
              key={`next-${step}`}
              className="btn primary"
              autoFocus={step !== "project"}
              onPress={() => {
                go(1);
              }}
            >
              {t("first-run-next")}
            </Button>
          )}
        </>
      }
    >
      <div className={styles.body} data-step={step}>
        {step === "workspace" && <WorkspaceStep />}
        {step === "look" && <LookStep />}
        {step === "project" && (
          <ProjectStep
            onCreated={() => {
              go(1);
            }}
          />
        )}
        {step === "agents" && <AgentsStep />}
      </div>
    </Dialog>
  );
}

function WorkspaceStep() {
  const t = useT();
  const workspace = useOp("workspace.get", {});
  const app = useAppSettings();
  const path = app.data?.workspace_path;
  return (
    <>
      <h2 className={styles.heading}>{t("first-run-welcome")}</h2>
      <p>{t("first-run-workspace", { name: workspace.data?.name ?? "" })}</p>
      {path !== undefined && (
        <p className={classes("selectable", styles.path)} data-testid="first-run-path">
          {path}
        </p>
      )}
      <p className="muted">{t("first-run-workspace-shared")}</p>
    </>
  );
}

function LookStep() {
  const t = useT();
  const { appearance, resolved, update } = useAppearance();
  return (
    <>
      <h2 className={styles.heading}>{t("first-run-look")}</h2>
      <ThemeTiles
        label={t("preferences-theme")}
        value={appearance.theme}
        accent={resolved.accent}
        systemThemes={[appearance.systemLight, appearance.systemDark]}
        onChange={(theme) => {
          update({ theme });
        }}
      />
      <div className={styles.line}>
        <span>{t("preferences-density")}</span>
        <Segmented
          label={t("preferences-density")}
          segments={DENSITIES.map((density) => ({ id: density, label: t(`density-${density}`) }))}
          value={appearance.density}
          onChange={(density) => {
            update({ density });
          }}
        />
      </div>
      <p className="muted">{t("first-run-look-later")}</p>
    </>
  );
}

function ProjectStep({ onCreated }: { onCreated: () => void }) {
  const t = useT();
  const call = useCall();
  const navigate = useNavigate();
  const projects = useOp("project.list", {});
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const count = projects.data?.items.length ?? 0;
  const key = suggestKey(name);
  const create = () => {
    if (name.trim() === "" || busy) {
      return;
    }
    setBusy(true);
    call("project.create", { name: name.trim(), key, parent: null }).then(
      (change) => {
        setBusy(false);
        void navigate({ to: "/projects/$key", params: { key: change.project.key } });
        onCreated();
      },
      (failure: unknown) => {
        setBusy(false);
        setError(failure instanceof OpFailure ? failure.error.message : String(failure));
      },
    );
  };
  return (
    <>
      <h2 className={styles.heading}>{t("first-run-project")}</h2>
      <p>{count === 0 ? t("first-run-project-about") : t("first-run-project-exists", { count })}</p>
      <form
        className={styles.line}
        onSubmit={(event) => {
          event.preventDefault();
          create();
        }}
      >
        <label className={classes("field", styles.name)}>
          <input
            // The step's own field takes the focus, so typing starts at once.
            autoFocus
            aria-label={t("project-name")}
            placeholder={t("first-run-project-placeholder")}
            value={name}
            onChange={(event) => {
              setName(event.target.value);
              setError(null);
            }}
          />
        </label>
        {key !== "" && <span className={classes("chip num", styles.key)}>{key}</span>}
        <Button type="submit" className="btn" isDisabled={name.trim() === "" || busy}>
          {t("project-create")}
        </Button>
      </form>
      {error !== null && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
      <p className="muted">{t("first-run-project-optional")}</p>
    </>
  );
}

function AgentsStep() {
  const t = useT();
  return (
    <>
      <h2 className={styles.heading}>{t("first-run-agents")}</h2>
      <p>{t("first-run-agents-about")}</p>
      <p className="muted">{t("first-run-agents-later")}</p>
    </>
  );
}
