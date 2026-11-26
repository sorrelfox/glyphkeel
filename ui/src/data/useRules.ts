// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useQueries } from "@tanstack/react-query";
import { useLocale } from "react-aria-components";

import type { Card, Project, Task } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { useCall } from "./hooks";
import { opKey } from "./queries";
import { fieldsOfCard, fieldsOfTask, missingFields, rulesOf, type KnownFields } from "./rules";

/**
 * The projects with these keys, with their statuses and rules, as the
 * project settings load them. A board with sub-projects holds cards of
 * several projects.
 */
export function useProjectsByKey(keys: readonly string[]): ReadonlyMap<string, Project> {
  const call = useCall();
  const unique = [...new Set(keys)].sort();
  const results = useQueries({
    queries: unique.map((key) => {
      const input = { project: key };
      return { queryKey: opKey("project.get", input), queryFn: () => call("project.get", input) };
    }),
  });
  const out = new Map<string, Project>();
  results.forEach((result, index) => {
    const key = unique[index];
    if (key !== undefined && result.data) {
      out.set(key, result.data);
      for (const old of result.data.old_keys ?? []) {
        out.set(old, result.data);
      }
    }
  });
  return out;
}

/**
 * Why a task can't enter a status yet, in the person's words, or undefined
 * when it can as far as the page can tell.
 */
export function useEntryReason(): (project: Project | undefined, statusId: string, fields: KnownFields) => string | undefined {
  const t = useT();
  const { locale } = useLocale();
  const list = new Intl.ListFormat(locale, { style: "long", type: "conjunction" });
  return (project, statusId, fields) => {
    const missing = missingFields(rulesOf(project, statusId), fields);
    if (missing.length === 0) {
      return undefined;
    }
    return t("rule-missing", { fields: list.format(missing.map((field) => t(`rule-field-${field}`))), count: missing.length });
  };
}

/** The fields a card or a loaded task tells. */
export function knownFields(item: Card | Task): KnownFields {
  return "description" in item ? fieldsOfTask(item) : fieldsOfCard(item);
}
