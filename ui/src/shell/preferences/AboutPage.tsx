// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowLeft, ExternalLink, FileText, Scale } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { useT } from "../../i18n/I18n";
import { LEGAL_FILES, useLegalText, type LegalText } from "../../ipc/legal";
import { openLink } from "../../ipc/links";
import { Group, Row } from "../PreferenceRows";
import styles from "../Preferences.module.css";

/** Where the source code is, as the licence asks the program to say. */
export const SOURCE_URL = "https://github.com/sorrelfox/glyphkeel";

/** The years of the copyright notice. */
const COPYRIGHT_YEARS = "2026";

/**
 * About: the version, the licence and where the source code is, and the
 * licence's text and the third-party notices, each read from the copy that
 * came with the app and shown in place of the page until Back.
 */
export function AboutPage() {
  const [showing, setShowing] = useState<LegalText | null>(null);
  if (showing !== null) {
    return (
      <TextView
        which={showing}
        onBack={() => {
          setShowing(null);
        }}
      />
    );
  }
  return <Summary onShow={setShowing} />;
}

function Summary({ onShow }: { onShow: (which: LegalText) => void }) {
  const t = useT();
  const toasts = useToasts();
  return (
    <>
      <Group title={t("about-group")}>
        <Row label={t("about-version")}>
          <span className="num selectable" data-testid="about-version">
            {__VERSION__}
          </span>
        </Row>
        <Row label={t("about-licence")} about={t("about-licence-about")}>
          <Button
            className="btn"
            onPress={() => {
              onShow("licence");
            }}
          >
            <Scale className="ic s" />
            {t("about-show-licence")}
          </Button>
        </Row>
        <Row label={t("about-source")} about={t("about-source-about")}>
          <Button
            className="btn"
            onPress={() => {
              openLink(SOURCE_URL).catch(() => {
                toasts.show({ tone: "error", text: t("about-source-failed") });
              });
            }}
          >
            <ExternalLink className="ic s" />
            {t("about-source-open")}
          </Button>
        </Row>
        <Row label={t("about-notices")} about={t("about-notices-about")}>
          <Button
            className="btn"
            onPress={() => {
              onShow("notices");
            }}
          >
            <FileText className="ic s" />
            {t("about-show-notices")}
          </Button>
        </Row>
      </Group>
      <p className={classes("faint", styles.aboutCopyright)}>{t("about-copyright", { years: COPYRIGHT_YEARS })}</p>
    </>
  );
}

function TextView({ which, onBack }: { which: LegalText; onBack: () => void }) {
  const t = useT();
  const text = useLegalText(which, true);
  const title = t(which === "licence" ? "about-licence-title" : "about-notices");
  return (
    <section className={styles.legal} aria-label={title}>
      <div className={styles.legalHeader}>
        <Button className="btn flat" onPress={onBack} autoFocus>
          <ArrowLeft className="ic s" />
          {t("about-back")}
        </Button>
        <h2 className={styles.groupTitle}>{title}</h2>
      </div>
      {text.isPending ? (
        <p className="faint" role="status">
          {t("about-loading")}
        </p>
      ) : text.isError ? (
        <p className={styles.legalMissing} role="alert">
          {text.error.error.message}
        </p>
      ) : text.data === null ? (
        <p className={styles.legalMissing} role="status">
          {t("about-text-elsewhere", { file: LEGAL_FILES[which] })}
        </p>
      ) : (
        // A long text in a box of its own: focusable, so it scrolls by keys too.
        <pre className={classes("selectable", styles.legalText)} tabIndex={0} aria-label={title} data-testid="legal-text" dir="ltr">
          {text.data}
        </pre>
      )}
    </section>
  );
}
