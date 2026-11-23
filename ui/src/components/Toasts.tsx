// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { createContext, use, useEffect, useRef, useState, type ReactNode } from "react";

import { useT } from "../i18n/I18n";

/** One short message at the bottom of the window. */
export interface Toast {
  tone: "info" | "error";
  text: string;
  /** A second line, such as what to do next. */
  detail?: string | undefined;
  /** One link button, such as Undo. */
  action?: { label: string; run: () => void } | undefined;
}

interface ShownToast extends Toast {
  id: number;
}

interface Toasts {
  show: (toast: Toast) => void;
}

/** How long a toast stays: errors and toasts with an action a little longer. */
const DURATION = { info: 5000, lasting: 8000 } as const;
/** Older toasts give way. */
const MAX_TOASTS = 3;

const ToastContext = createContext<Toasts | null>(null);

/** Shows toasts for everything below it. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ShownToast[]>([]);
  const next = useRef(1);
  const timers = useRef(new Set<ReturnType<typeof setTimeout>>());

  useEffect(() => {
    const pending = timers.current;
    return () => {
      for (const timer of pending) {
        clearTimeout(timer);
      }
    };
  }, []);

  const dismiss = (id: number) => {
    setToasts((shown) => shown.filter((toast) => toast.id !== id));
  };
  const [api] = useState<Toasts>(() => ({
    show(toast) {
      const id = next.current++;
      setToasts((shown) => [...shown, { ...toast, id }].slice(-MAX_TOASTS));
      const lasting = toast.tone === "error" || toast.action !== undefined;
      const timer = setTimeout(() => {
        timers.current.delete(timer);
        dismiss(id);
      }, lasting ? DURATION.lasting : DURATION.info);
      timers.current.add(timer);
    },
  }));

  return (
    <ToastContext value={api}>
      {children}
      <ToastRegion toasts={toasts} onDismiss={dismiss} />
    </ToastContext>
  );
}

function ToastRegion({ toasts, onDismiss }: { toasts: ShownToast[]; onDismiss: (id: number) => void }) {
  const t = useT();
  return (
    // A top layer for React Aria: a modal dialog leaves it readable and usable.
    <div className="toasts" aria-label={t("toast-region")} role="region" data-react-aria-top-layer="">
      {toasts.map((toast) => (
        <div key={toast.id} className="toast" role={toast.tone === "error" ? "alert" : "status"} data-tone={toast.tone}>
          <span className="toastText">
            {toast.text}
            {toast.detail && <span className="toastDetail">{toast.detail}</span>}
          </span>
          {toast.action && (
            <button
              type="button"
              className="toastAction"
              onClick={() => {
                toast.action?.run();
                onDismiss(toast.id);
              }}
            >
              {toast.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

/** Shows a toast. Outside a provider, toasts go to the console. */
export function useToasts(): Toasts {
  return use(ToastContext) ?? consoleToasts;
}

const consoleToasts: Toasts = {
  show: (toast) => {
    console.warn(toast.text, toast.detail ?? "");
  },
};
