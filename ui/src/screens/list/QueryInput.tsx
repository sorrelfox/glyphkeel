// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useLocalization } from "@fluent/react";
import { useEffect, useRef } from "react";

import { QueryField, type QueryFieldHandle } from "../../components/QueryField/QueryField";
import { useListNames, useParsedQuery, useQueryFields } from "../../data/list";
import { type QueryProblem } from "../../generated/ops";
import { Chips } from "./Chips";
import { removeRanges } from "./model";

/** The engine's message for a query mistake, in the person's language. */
export function useProblemText(): (problem: QueryProblem) => string {
  const { l10n } = useLocalization();
  return (problem) => (l10n.getBundle(problem.message_id) ? l10n.getString(problem.message_id, problem.args ?? null) : problem.message);
}

interface QueryInputProps {
  /** The project whose names complete values; none: the whole workspace's. */
  project: string | undefined;
  value: string;
  onChange: (text: string) => void;
  label: string;
  placeholder: string;
  /** The ID of a hint that describes the field. */
  describedBy?: string | undefined;
  /** Enter, when no completion is open. */
  onSubmit?: () => void;
  /** Shows the conditions as chips under the field. */
  chips?: boolean;
  /** Takes the focus when it first shows. */
  autoFocus?: boolean;
  /** A mistake found elsewhere, such as on saving; shown instead of the one found while typing. */
  problem?: string | undefined;
}

/**
 * A query field anywhere a query is typed: the list's field with the
 * engine's colours, completion of fields and values with their help, the
 * mistake under it as it is typed, and, when asked, the conditions as chips.
 */
export function QueryInput({ project, value, onChange, label, placeholder, describedBy, onSubmit, chips = false, autoFocus = false, problem: given }: QueryInputProps) {
  const parsed = useParsedQuery(value);
  const reading = parsed.data && !parsed.isPlaceholderData ? parsed.data : undefined;
  const names = useListNames(project);
  const fields = useQueryFields();
  const problemText = useProblemText();
  const field = useRef<QueryFieldHandle>(null);
  const found = value.trim() === "" ? undefined : reading?.error;
  const problem = given ?? (found ? problemText(found) : undefined);

  useEffect(() => {
    if (autoFocus) {
      field.current?.focus();
    }
    // Only when it first shows.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <>
      <QueryField
        ref={field}
        value={value}
        onChange={onChange}
        reading={reading ? { text: value, tokens: reading.tokens, error: reading.error } : undefined}
        problem={problem}
        fields={fields}
        names={names}
        label={label}
        placeholder={placeholder}
        {...(describedBy === undefined ? {} : { describedBy })}
        {...(onSubmit ? { onSubmit } : {})}
      />
      {chips && (
        <Chips
          chips={reading?.valid === true ? reading.chips : undefined}
          advanced={reading?.advanced ?? false}
          onRemove={(chip) => {
            onChange(removeRanges(value, chip.ranges));
          }}
          onAdd={() => field.current?.addCondition()}
        />
      )}
    </>
  );
}
