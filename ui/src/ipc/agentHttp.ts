// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The local HTTP endpoint for agents whose client can't start a program.
// The app keeps its settings with the person's other settings (the
// command line's `glyphkeel mcp --http` reads the same token), serves it on
// 127.0.0.1 and asks in a window of the desktop before turning it on. In a
// plain browser (the development bridge, the sample data, tests) nothing
// listens: the page keeps the settings in its storage so the set-up can be
// shown and tried.

import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

import { useToasts } from "../components/Toasts";
import { errorToast } from "../data/hooks";
import { type CallError } from "../generated/ops";
import { OpFailure, toCallError } from "../data/transport";
import { useT } from "../i18n/I18n";
import { readPref, writePref } from "../prefs/store";
import { inApp } from "./files";

export interface AgentHttp {
  /** Whether the person turned it on. */
  enabled: boolean;
  /** The chosen port; absent: a free one each start. */
  port?: number;
  /** Where it serves now; absent while it doesn't. */
  url?: string;
  /** The token clients send; absent until it is first turned on. */
  token?: string;
  /** Why it doesn't serve although it is on. */
  error?: CallError;
}

export interface AgentHttpPatch {
  enabled?: boolean;
  /** 1024 to 65535; 0 for a free port each start. */
  port?: number;
}

/** The lowest port the person may choose. */
export const PORT_MIN = 1024;
export const PORT_MAX = 65535;

const PREF = "agent-http";
/** The port shown outside the app while none is chosen. */
const SAMPLE_PORT = 49152;

/** 256 random bits as hex, as the app makes them. */
function newToken(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** The settings kept in the page's storage, outside the app. */
function stored(): AgentHttp {
  const value = readPref(PREF);
  const record = typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
  const enabled = record.enabled === true;
  const port = typeof record.port === "number" ? record.port : undefined;
  const token = typeof record.token === "string" ? record.token : undefined;
  return {
    enabled,
    ...(port === undefined ? {} : { port }),
    ...(token === undefined ? {} : { token }),
    ...(enabled ? { url: `http://127.0.0.1:${String(port ?? SAMPLE_PORT)}/mcp` } : {}),
  };
}

function keep(settings: { enabled: boolean; port?: number | undefined; token?: string | undefined }): AgentHttp {
  writePref(PREF, { enabled: settings.enabled, port: settings.port, token: settings.token });
  return stored();
}

async function command(name: string, args: Record<string, unknown> = {}): Promise<AgentHttp> {
  try {
    return await invoke<AgentHttp>(name, args);
  } catch (error: unknown) {
    throw new OpFailure(name, toCallError(error));
  }
}

export function loadAgentHttp(): Promise<AgentHttp> {
  return inApp() ? command("agent_http") : Promise.resolve(stored());
}

export function updateAgentHttp(patch: AgentHttpPatch): Promise<AgentHttp> {
  if (inApp()) {
    return command("update_agent_http", { patch });
  }
  if (patch.port !== undefined && patch.port !== 0 && (patch.port < PORT_MIN || patch.port > PORT_MAX)) {
    return Promise.reject(
      new OpFailure("update_agent_http", {
        code: "invalid",
        message: `Port ${String(patch.port)} can't be used.`,
        details: { message_id: "agent-http-port-range", args: { port: String(patch.port) }, path: "port" },
      }),
    );
  }
  const now = stored();
  const enabled = patch.enabled ?? now.enabled;
  const port = patch.port === undefined ? now.port : patch.port === 0 ? undefined : patch.port;
  const token = enabled && now.token === undefined ? newToken() : now.token;
  return Promise.resolve(keep({ enabled, port, token }));
}

export function regenerateAgentToken(): Promise<AgentHttp> {
  if (inApp()) {
    return command("regenerate_agent_token");
  }
  return Promise.resolve(keep({ ...stored(), token: newToken() }));
}

/** Whether something really listens: only the app serves the endpoint. */
export function servesAgentHttp(): boolean {
  return inApp();
}

/** The set-up to copy into a client that connects over HTTP. */
export function httpSnippet(url: string, token: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        glyphkeel: { type: "http", url, headers: { Authorization: `Bearer ${token}` } },
      },
    },
    null,
    2,
  );
}

const KEY = ["agent-http"] as const;

/** The endpoint's settings and state. */
export function useAgentHttp(): UseQueryResult<AgentHttp, OpFailure> {
  return useQuery({ queryKey: KEY, queryFn: loadAgentHttp, staleTime: Infinity });
}

/** Changes the endpoint (`update`) or makes a new token (`regenerate`); errors become a toast. */
export function useChangeAgentHttp(): { update: (patch: AgentHttpPatch) => void; regenerate: () => void; isPending: boolean } {
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const done = (settings: AgentHttp) => {
    queryClient.setQueryData(KEY, settings);
  };
  const fail = (error: OpFailure) => {
    toasts.show(errorToast(t, error));
  };
  const updating = useMutation({ mutationFn: updateAgentHttp, onSuccess: done, onError: fail });
  const regenerating = useMutation({ mutationFn: regenerateAgentToken, onSuccess: done, onError: fail });
  return {
    update: (patch) => {
      updating.mutate(patch);
    },
    regenerate: () => {
      regenerating.mutate();
    },
    isPending: updating.isPending || regenerating.isPending,
  };
}
