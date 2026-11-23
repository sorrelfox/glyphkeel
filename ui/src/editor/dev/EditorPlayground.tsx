// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowLeft } from "lucide-react";
import { Suspense, useState } from "react";
import { Button } from "react-aria-components";

import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { useT } from "../../i18n/I18n";
import { MarkdownEditor, type ChecklistToggle, type CompletionProviders, type EditorMode } from "../index";
import styles from "./EditorPlayground.module.css";
import { SAMPLES, TASKS, PAGES, type SampleId } from "./samples";

const providers: CompletionProviders = {
  tasks: (project, query) => {
    const q = query.toLowerCase();
    return TASKS.filter((task) => task.key.startsWith(`${project}-`))
      .filter((task) => !q || task.key.slice(project.length + 1).startsWith(q) || task.title.toLowerCase().includes(q))
      .slice(0, 20);
  },
  pages: (query) => PAGES.filter((page) => page.toLowerCase().includes(query.toLowerCase())),
};

/** What the engine would do with a toggle: change the one character, by its offset in the text. */
function engineToggle(text: string, toggle: ChecklistToggle): string {
  const at = toggle.offset + 1;
  return text.slice(0, at) + (toggle.checked ? " " : "x") + text.slice(at + 1);
}

/**
 * The development page of the Markdown editor: every widget, completion,
 * the paste rules and the line-break rules, on sample texts. The browser
 * tests drive it; only developers see it.
 */
export default function EditorPlayground({ onLeave }: { onLeave: () => void }) {
  const t = useT();
  const [sample, setSample] = useState<SampleId>("sample");
  const [texts, setTexts] = useState<Record<SampleId, string>>(() => ({ ...SAMPLES }));
  const [mode, setMode] = useState<EditorMode>("preview");
  const [engineToggles, setEngineToggles] = useState(false);
  const [toggles, setToggles] = useState<ChecklistToggle[]>([]);
  const value = texts[sample];

  const setValue = (next: string) => {
    setTexts((all) => ({ ...all, [sample]: next }));
  };

  return (
    <div className={styles.page}>
      <div className={styles.toolbar}>
        <Button className="btn flat sm" onPress={onLeave} aria-label={t("editor-dev-back")}>
          <ArrowLeft size={16} aria-hidden="true" />
        </Button>
        <h1 className={styles.title}>{t("editor-dev-title")}</h1>
        <Segmented
          label={t("editor-dev-sample")}
          value={sample}
          onChange={setSample}
          segments={(Object.keys(SAMPLES) as SampleId[]).map((id) => ({ id, label: id }))}
        />
        <Segmented
          label={t("editor-dev-mode")}
          value={mode}
          onChange={setMode}
          segments={[
            { id: "preview", label: t("editor-dev-preview") },
            { id: "source", label: t("editor-dev-source") },
          ]}
        />
        <Switch isSelected={engineToggles} onChange={setEngineToggles}>
          {t("editor-dev-engine-toggles")}
        </Switch>
      </div>
      <div className={styles.frame}>
        <Suspense>
          <MarkdownEditor
            key={sample}
            className={styles.editor}
            label={t("editor-dev-title")}
            value={value}
            onChange={setValue}
            mode={mode}
            completion={providers}
            {...(engineToggles
              ? {
                  onToggleChecklist: (toggle: ChecklistToggle) => {
                    setToggles((list) => [...list, toggle]);
                    setValue(engineToggle(value, toggle));
                  },
                }
              : {})}
          />
        </Suspense>
      </div>
      {/* What the tests read: the text exactly as the editor reports it, and the engine's toggles. */}
      <output hidden data-testid="editor-text" data-length={value.length}>
        {value.length < 20_000 ? JSON.stringify(value) : ""}
      </output>
      <output hidden data-testid="editor-toggles">
        {JSON.stringify(toggles)}
      </output>
    </div>
  );
}
