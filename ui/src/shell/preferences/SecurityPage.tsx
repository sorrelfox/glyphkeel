// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useRef, useState, type ReactNode, type SyntheticEvent } from "react";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { Select } from "../../components/Select";
import { failureText } from "../../data/hooks";
import { useT } from "../../i18n/I18n";
import {
  changeEncryption,
  lockApp,
  updateLock,
  useLock,
  useLockMutation,
  type EncryptionChanged,
  type EncryptionRequest,
  type LockPatch,
} from "../../ipc/lock";
import styles from "../lock/Lock.module.css";
import { Group, Row } from "../PreferenceRows";

/** The idle times to choose from, in minutes; 0 is never. */
const IDLE_CHOICES = ["0", "1", "5", "10", "15", "30", "60"] as const;
type IdleChoice = (typeof IDLE_CHOICES)[number];

/** A passphrase field with its label above it. */
function PassphraseField({
  label,
  value,
  onChange,
  autoComplete = "new-password",
  first = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoComplete?: string;
  /** The form's first field, which takes the focus when the form opens. */
  first?: boolean;
}) {
  return (
    <label className={styles.label}>
      <span>{label}</span>
      <span className="field">
        <input
          type="password"
          autoFocus={first}
          autoComplete={autoComplete}
          value={value}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      </span>
    </label>
  );
}

/** A small form inside a group: its fields, an error, and its buttons. */
function InlineForm({ label, onSubmit, error, children, submit, onCancel, busy }: { label: string; onSubmit: () => void; error: string | null; children: ReactNode; submit: string; onCancel: () => void; busy: boolean }) {
  const t = useT();
  return (
    <form
      className={styles.fields}
      aria-label={label}
      onSubmit={(event: SyntheticEvent) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      {children}
      {error !== null && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
      <span className={styles.actions}>
        <button type="button" className="btn flat" onClick={onCancel}>
          {t("security-cancel")}
        </button>
        <button type="submit" className={classes("btn", "primary")} disabled={busy}>
          {submit}
        </button>
      </span>
    </form>
  );
}

type EncryptionForm = "encrypt" | "change" | "decrypt" | null;

/**
 * Security: the workspace's encryption (encrypt, change the passphrase,
 * decrypt) and the app lock
 * (when it locks by itself, its passphrase for a plain workspace, lock
 * now).
 */
export function SecurityPage() {
  const t = useT();
  const toasts = useToasts();
  const lock = useLock();
  const view = lock.data;
  const encryption = view?.encryption;

  const [form, setForm] = useState<EncryptionForm>(null);
  // Where the focus goes back to when a form closes: the button that
  // opens the forms of that group, drawn again.
  const giveFocus = useRef<"encryption" | "lock" | null>(null);
  const takeFocus = (group: "encryption" | "lock") => (button: HTMLButtonElement | null) => {
    if (button !== null && giveFocus.current === group) {
      giveFocus.current = null;
      button.focus();
    }
  };
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const [error, setError] = useState<string | null>(null);
  const reset = () => {
    giveFocus.current = "encryption";
    setForm(null);
    setCurrent("");
    setNext("");
    setAgain("");
    setError(null);
  };
  const change = useLockMutation((request: EncryptionRequest) => changeEncryption(request));

  const [lockForm, setLockForm] = useState(false);
  const [lockCurrent, setLockCurrent] = useState("");
  const [lockNext, setLockNext] = useState("");
  const [lockAgain, setLockAgain] = useState("");
  const [lockError, setLockError] = useState<string | null>(null);
  const resetLock = () => {
    giveFocus.current = "lock";
    setLockForm(false);
    setLockCurrent("");
    setLockNext("");
    setLockAgain("");
    setLockError(null);
  };
  const update = useLockMutation((patch: LockPatch) => updateLock(patch));
  const lockNow = useLockMutation(() => lockApp());

  const submitEncryption = () => {
    if ((form === "encrypt" || form === "change") && next !== again) {
      setError(t("security-mismatch"));
      return;
    }
    const request: EncryptionRequest =
      form === "decrypt" ? { encrypt: false, current } : { encrypt: true, passphrase: next, ...(form === "change" ? { current } : {}) };
    change.mutate(request, {
      onSuccess: (changed) => {
        const done = changed as EncryptionChanged;
        reset();
        toasts.show({
          tone: "info",
          text: t(form === "decrypt" ? "security-decrypted" : form === "change" ? "security-changed" : "security-encrypted", { backup: done.backup }),
          detail:
            done.files_failed > 0
              ? t("security-files-left", { count: done.files_failed })
              : done.backups_left_plain > 0
                ? t("security-backups-left-plain", { count: done.backups_left_plain })
                : undefined,
        });
      },
      onError: (failure) => {
        setError(failureText(t, failure));
      },
    });
  };

  const submitLock = () => {
    if (lockNext !== lockAgain) {
      setLockError(t("security-mismatch"));
      return;
    }
    update.mutate(
      { lock_passphrase: lockNext, ...(view?.lock_passphrase === true ? { current: lockCurrent } : {}) },
      {
        onSuccess: resetLock,
        onError: (failure) => {
          setLockError(failureText(t, failure));
        },
      },
    );
  };

  const idleValue: IdleChoice = (IDLE_CHOICES as readonly string[]).includes(String(view?.lock_after_minutes ?? 0)) ? (String(view?.lock_after_minutes ?? 0) as IdleChoice) : "0";
  const encrypted = encryption?.encrypted === true;
  const busy = change.isPending;

  return (
    <>
      <Group title={t("security-encryption")} lead={<p className={styles.warning}>{t("security-encryption-lead")}</p>}>
        <Row
          label={t("security-encryption-state")}
          about={encryption === undefined ? undefined : !encryption.available ? t("security-unavailable") : encrypted ? t("security-is-encrypted") : t("security-is-plain")}
        >
          {encryption?.available === true && form === null && (
            <span className={styles.actions}>
              {encrypted ? (
                <>
                  <button type="button" className="btn" ref={takeFocus("encryption")} onClick={() => { setForm("change"); }}>
                    {t("security-change")}
                  </button>
                  <button type="button" className="btn" onClick={() => { setForm("decrypt"); }}>
                    {t("security-decrypt")}
                  </button>
                </>
              ) : (
                <button type="button" className="btn" ref={takeFocus("encryption")} onClick={() => { setForm("encrypt"); }}>
                  {t("security-encrypt")}
                </button>
              )}
            </span>
          )}
        </Row>
        {(form === "encrypt" || form === "change" || form === "decrypt") && (
          <InlineForm
            label={t(`security-${form}`)}
            submit={t(`security-${form}-submit`)}
            onSubmit={submitEncryption}
            onCancel={reset}
            error={error}
            busy={busy}
          >
            <p className={styles.warning}>{t(form === "decrypt" ? "security-decrypt-warning" : "security-encrypt-warning")}</p>
            {form !== "encrypt" && <PassphraseField first label={t("security-current")} value={current} onChange={setCurrent} autoComplete="current-password" />}
            {form !== "decrypt" && (
              <>
                <PassphraseField first={form === "encrypt"} label={t("security-new")} value={next} onChange={setNext} />
                <PassphraseField label={t("security-again")} value={again} onChange={setAgain} />
              </>
            )}
          </InlineForm>
        )}
      </Group>

      <Group title={t("security-lock")}>
        <Row label={t("security-lock-after")} about={view?.can_lock === true ? t("security-lock-after-about") : t("security-lock-needs-passphrase")}>
          <Select
            label={t("security-lock-after")}
            isDisabled={view?.can_lock !== true}
            value={idleValue}
            options={IDLE_CHOICES.map((id) => ({ id, label: id === "0" ? t("security-lock-never") : t("security-lock-minutes", { minutes: Number(id) }) }))}
            onChange={(value) => {
              update.mutate({ lock_after_minutes: Number(value) });
            }}
          />
        </Row>
        {!encrypted && (
          <Row label={t("security-lock-passphrase")} about={t(view?.lock_passphrase === true ? "security-lock-passphrase-set" : "security-lock-passphrase-about")}>
            {!lockForm && (
              <span className={styles.actions}>
                <button type="button" className="btn" ref={takeFocus("lock")} onClick={() => { setLockForm(true); }}>
                  {t(view?.lock_passphrase === true ? "security-lock-passphrase-change" : "security-lock-passphrase-set-button")}
                </button>
              </span>
            )}
          </Row>
        )}
        {!encrypted && lockForm && (
          <InlineForm label={t("security-lock-passphrase")} submit={t("security-lock-passphrase-submit")} onSubmit={submitLock} onCancel={resetLock} error={lockError} busy={update.isPending}>
            {view?.lock_passphrase === true && <PassphraseField first label={t("security-current")} value={lockCurrent} onChange={setLockCurrent} autoComplete="current-password" />}
            <PassphraseField first={view?.lock_passphrase !== true} label={t("security-new")} value={lockNext} onChange={setLockNext} />
            <PassphraseField label={t("security-again")} value={lockAgain} onChange={setLockAgain} />
            {view?.lock_passphrase === true && <p className={styles.warning}>{t("security-lock-passphrase-remove-hint")}</p>}
          </InlineForm>
        )}
        <Row label={t("security-lock-now")} about={t("security-lock-now-about")}>
          <button
            type="button"
            className="btn"
            disabled={view?.can_lock !== true}
            onClick={() => {
              lockNow.mutate(undefined);
            }}
          >
            {t("security-lock-now-button")}
          </button>
        </Row>
      </Group>
    </>
  );
}
