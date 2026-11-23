// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createContext, use, useEffect, useState, type ReactNode } from "react";

import { applyEvent, refetchAll } from "./events";
import { type Client } from "./transport";

/**
 * Answers stay cached until a change event says otherwise: the engine
 * announces every commit, its own and other programs', so nothing is
 * refetched on a timer, on focus or on a retry.
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { staleTime: Infinity, retry: false, refetchOnWindowFocus: false, refetchOnReconnect: false },
      mutations: { retry: false },
    },
  });
}

const ClientContext = createContext<Client | null>(null);

interface DataProviderProps {
  client: Client;
  /** A cache of its own, for tests. */
  queryClient?: QueryClient;
  children: ReactNode;
}

/**
 * The engine's data for everything below: the typed client, the query cache,
 * and the change events that keep the cache current.
 */
export function DataProvider({ client, queryClient: given, children }: DataProviderProps) {
  const [queryClient] = useState(() => given ?? createQueryClient());

  useEffect(() => {
    let stop: (() => void) | undefined;
    let active = true;
    client.subscribe((event) => void applyEvent(queryClient, event)).then(
      (unsubscribe) => {
        if (!active) {
          unsubscribe();
          return;
        }
        stop = unsubscribe;
        // Whatever changed before the events flowed is read again.
        void refetchAll(queryClient);
      },
      (error: unknown) => {
        console.error("change events are not available", error);
      },
    );
    return () => {
      active = false;
      stop?.();
    };
  }, [client, queryClient]);

  return (
    <ClientContext value={client}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </ClientContext>
  );
}

/** The typed client of the engine. */
export function useClient(): Client {
  const client = use(ClientContext);
  if (!client) {
    throw new Error("useClient needs a DataProvider above it");
  }
  return client;
}
