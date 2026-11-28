// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The digest: what agents, automation rules, reminders and the command line
// did since the person last dismissed it, and the sidebar's count of it.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { type Digest } from "../generated/ops";
import { useCall } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

/** The input of every digest read, so that Today and the sidebar share one answer. */
const DIGEST_INPUT = {};

/** The digest since the person's marker; read again on changes, and every few minutes for due dates. */
export function useDigest() {
  const call = useCall();
  return useQuery<Digest, OpFailure>({
    queryKey: opKey("digest.get", DIGEST_INPUT),
    queryFn: () => call("digest.get", DIGEST_INPUT),
    refetchInterval: 5 * 60_000,
  });
}

/** The sidebar's count next to Inbox: the digest's entries. */
export function useInboxCount(): number | undefined {
  return useDigest().data?.counts.items;
}

/**
 * Marks the digest as seen up to the moment it was read (`until`), so that
 * what came in while the person read it shows next time. Moving the marker
 * is no change to the workspace, so no change event reads the digest
 * again: this does.
 */
export function useDismissDigest() {
  const call = useCall();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (until: string) => call("digest.dismiss", { at: until }),
    onSettled: () => queryClient.invalidateQueries({ queryKey: opKey("digest.get", DIGEST_INPUT) }),
  });
}
