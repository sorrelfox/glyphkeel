// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Custom fields: the fields a project's tasks have, and their values as
// people read them. Field names and options are the person's own words and
// are shown as written, never translated.

import { useLocale } from "react-aria-components";

import { type CustomField, type CustomFieldKind } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { useOp } from "./hooks";

/** Every kind of field, in the order the person is offered them. */
export const FIELD_KINDS: readonly CustomFieldKind[] = ["text", "markdown", "integer", "decimal", "date", "boolean", "select", "multi_select", "url"];

/** Kinds whose values are options of the field's list. */
export const OPTION_KINDS: ReadonlySet<CustomFieldKind> = new Set(["select", "multi_select"]);

/** Kinds whose values are numbers, aligned at the end in tables. */
export const NUMBER_KINDS: ReadonlySet<CustomFieldKind> = new Set(["integer", "decimal"]);

/** A value as the engine gives it. */
export type FieldValue = string | number | boolean | string[];

/** The custom fields of a project (its own and those of every project), or of the whole workspace. */
export function useCustomFields(project: string | undefined): CustomField[] {
  const fields = useOp("field.list", project === undefined ? {} : { project });
  return fields.data?.fields ?? [];
}

/** Any value as plain text: text as it is, numbers and yes/no as written, anything else as JSON. */
export function plainText(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return value === undefined || value === null ? "" : JSON.stringify(value);
}

/** Whether a field shows for tasks of a type. */
export function shownFor(field: Pick<CustomField, "types">, type: string): boolean {
  const types = field.types ?? [];
  return types.length === 0 || types.some((name) => name.localeCompare(type, undefined, { sensitivity: "base" }) === 0);
}

const formats = new Map<string, Intl.NumberFormat | Intl.DateTimeFormat | Intl.ListFormat>();

function cached<T extends Intl.NumberFormat | Intl.DateTimeFormat | Intl.ListFormat>(key: string, make: () => T): T {
  const found = formats.get(key);
  if (found !== undefined) {
    return found as T;
  }
  const made = make();
  formats.set(key, made);
  return made;
}

/** A value as people read it, in their locale: numbers and dates formatted, yes/no translated, options listed. */
export function useFieldText(): (kind: CustomFieldKind, value: unknown) => string {
  const t = useT();
  const { locale } = useLocale();
  return (kind, value) => {
    if (value === undefined || value === null) {
      return "";
    }
    switch (kind) {
      case "integer":
      case "decimal":
        return typeof value === "number" ? cached(`${locale}|n`, () => new Intl.NumberFormat(locale, { maximumFractionDigits: 6 })).format(value) : plainText(value);
      case "date": {
        const day = plainText(value);
        return /^\d{4}-\d{2}-\d{2}$/.test(day) ? cached(`${locale}|d`, () => new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" })).format(new Date(`${day}T00:00:00Z`)) : day;
      }
      case "boolean":
        return t(value === true ? "field-yes" : "field-no");
      case "multi_select":
        return Array.isArray(value) ? cached(`${locale}|l`, () => new Intl.ListFormat(locale, { style: "narrow", type: "unit" })).format(value.map(plainText)) : plainText(value);
      case "markdown":
        // One line of the text, for a cell.
        return plainText(value).replace(/\s+/g, " ").trim();
      case "text":
      case "select":
      case "url":
        return plainText(value);
    }
  };
}

/** A value for a CSV file: locale-neutral, as the engine writes it; options joined by "; ". */
export function csvFieldText(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  if (Array.isArray(value)) {
    return value.map(plainText).join("; ");
  }
  return plainText(value);
}
