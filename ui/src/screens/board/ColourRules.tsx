// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { useToasts } from "../../components/Toasts";
import { failureText, useCall, useUndo } from "../../data/hooks";
import { type ColourRule } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { QueryInput } from "../list/QueryInput";
import { HuePicker } from "../project/HuePicker";
import styles from "./ColourRules.module.css";

/** The most rules the engine keeps. */
const MAX_RULES = 20;

/** A rule being edited, with a key that stays when rules move. */
interface Draft extends ColourRule {
  key: number;
}

let drafts = 0;

function draft(rule: Partial<ColourRule> = {}): Draft {
  drafts += 1;
  return { key: drafts, query: rule.query ?? "", hue: rule.hue ?? "blue", name: rule.name ?? "" };
}

/** Which rule an engine error is about: its index in `colour_rules[i].query`. */
function ruleOf(error: unknown): number | null {
  const path = (error as { error?: { details?: { path?: unknown } } } | null)?.error?.details?.path;
  const found = typeof path === "string" ? /^colour_rules\[(\d+)\]/.exec(path) : null;
  return found ? Number(found[1]) : null;
}

interface ColourRulesDialogProps {
  project: string;
  rules: readonly ColourRule[];
  onClose: () => void;
}

/**
 * The board's colour rules, in order: each a name, a query and a colour.
 * Saving turns colouring by rules on. The engine checks each query; a
 * mistake stays in the dialog, under the rule it is about.
 */
export function ColourRulesDialog({ project, rules, onClose }: ColourRulesDialogProps) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const [list, setList] = useState<Draft[]>(() => (rules.length === 0 ? [draft()] : rules.map((rule) => draft(rule))));
  const [error, setError] = useState<{ rule: number | null; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const change = (at: number, patch: Partial<ColourRule>) => {
    setList((before) => before.map((rule, index) => (index === at ? { ...rule, ...patch } : rule)));
    setError(null);
  };
  const move = (at: number, by: number) => {
    setList((before) => {
      const next = [...before];
      const [rule] = next.splice(at, 1);
      if (rule) {
        next.splice(at + by, 0, rule);
      }
      return next;
    });
    setError(null);
  };
  const save = () => {
    // Rules left without a query are dropped, not refused.
    const kept = list.filter((rule) => rule.query.trim() !== "");
    setBusy(true);
    call("layout.update", {
      project,
      colour_by: "query",
      colour_rules: kept.map(({ query, hue, name }) => ({ query: query.trim(), hue, ...(name?.trim() ? { name: name.trim() } : {}) })),
    }).then(
      () => {
        toasts.show({ tone: "info", text: t("colour-rules-saved"), action: { label: t("toast-undo"), run: undo } });
        onClose();
      },
      (failure: unknown) => {
        setBusy(false);
        const index = ruleOf(failure);
        // The index is among the kept rules; show it on the rule as listed.
        const shown = index === null ? null : list.indexOf(kept[index] as Draft);
        setError({ rule: shown === -1 ? null : shown, text: failureText(t, failure) });
      },
    );
  };

  return (
    <Dialog
      title={t("colour-rules-title")}
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
          <Button className="btn primary" isDisabled={busy} onPress={save}>
            {t("colour-rules-save")}
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
        <p className={styles.intro}>{t("colour-rules-intro")}</p>
        {list.length === 0 && <p className="muted">{t("colour-rules-none")}</p>}
        <ol className={styles.rules}>
          {list.map((rule, at) => {
            const number = at + 1;
            const wrong = error !== null && error.rule === at;
            return (
              <li key={rule.key} className={classes(styles.rule, wrong && styles.wrong)} aria-label={t("colour-rules-rule", { number })} data-rule={number}>
                <div className={styles.ruleFields}>
                  <label className={styles.row}>
                    <span className={styles.label}>{t("colour-rules-name")}</span>
                    <span className="field">
                      <input
                        value={rule.name ?? ""}
                        maxLength={60}
                        placeholder={t("colour-rules-name-placeholder")}
                        onChange={(event) => {
                          change(at, { name: event.target.value });
                        }}
                      />
                    </span>
                  </label>
                  <div className={styles.row}>
                    <span className={styles.label} aria-hidden="true">
                      {t("colour-rules-query")}
                    </span>
                    <div className={styles.query}>
                      <QueryInput
                        project={project}
                        value={rule.query}
                        onChange={(query) => {
                          change(at, { query });
                        }}
                        label={t("colour-rules-query")}
                        placeholder={t("colour-rules-query-placeholder")}
                        autoFocus={at === list.length - 1 && rule.query === ""}
                        // The engine's mistake on saving, under the rule it is about.
                        problem={wrong ? error.text : undefined}
                      />
                    </div>
                  </div>
                  <div className={styles.row}>
                    <span className={styles.label} aria-hidden="true">
                      {t("colour-rules-hue")}
                    </span>
                    <HuePicker
                      label={t("colour-rules-hue")}
                      value={rule.hue}
                      onChange={(hue) => {
                        change(at, { hue });
                      }}
                    />
                  </div>
                </div>
                <div className={styles.ruleButtons}>
                  <Button
                    className="btn flat icon sm"
                    aria-label={t("colour-rules-up", { number })}
                    isDisabled={at === 0}
                    onPress={() => {
                      move(at, -1);
                    }}
                  >
                    <ArrowUp className="ic s" />
                  </Button>
                  <Button
                    className="btn flat icon sm"
                    aria-label={t("colour-rules-down", { number })}
                    isDisabled={at === list.length - 1}
                    onPress={() => {
                      move(at, 1);
                    }}
                  >
                    <ArrowDown className="ic s" />
                  </Button>
                  <Button
                    className="btn flat icon sm"
                    aria-label={t("colour-rules-remove", { number })}
                    onPress={() => {
                      setList((before) => before.filter((_, index) => index !== at));
                      setError(null);
                    }}
                  >
                    <Trash2 className="ic s" />
                  </Button>
                </div>
              </li>
            );
          })}
        </ol>
        {error !== null && error.rule === null && (
          <p className={styles.error} role="alert">
            {error.text}
          </p>
        )}
        <div>
          <Button
            className="btn flat"
            isDisabled={list.length >= MAX_RULES}
            onPress={() => {
              setList((before) => [...before, draft()]);
            }}
          >
            <Plus className="ic s" />
            <span>{t("colour-rules-add")}</span>
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
