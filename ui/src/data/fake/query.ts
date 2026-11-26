// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The query language as the sample workspace understands it: enough of the
// engine's grammar for the list screen to be worked on without the engine.
// Conditions joined by spaces (AND), OR, parentheses, "-" for NOT, quoted
// phrases, `field:value,value`, comparisons, `sort:`. Errors carry the
// engine's message IDs, and every range is in UTF-16 units, as the engine's.

import {
  type CustomFieldKind,
  type Hue,
  type ListedTask,
  type ListGroup,
  type ListGrouping,
  type ListTotals,
  type ParsedQuery,
  type QueryChip,
  type QueryField,
  type QueryProblem,
  type QueryToken,
  type QueryTokenKind,
  type SavedView,
  type TaskSearchInput,
  type TaskSearchResult,
  type TextRange,
  type ViewCreateInput,
  type ViewDeleteInput,
  type ViewListInput,
  type ViewUpdateInput,
} from "../../generated/ops";
import { parseDuration } from "../duration";

/** `{ [key]: value }` when there is a value; nothing otherwise. */
function some<K extends string, V>(key: K, value: V | null | undefined): { [P in K]?: V } {
  return value === null || value === undefined ? {} : ({ [key]: value } as { [P in K]?: V });
}

/** The names a query may use, from the workspace. */
export interface QueryNames {
  statuses: readonly string[];
  types: readonly string[];
  labels: readonly string[];
  projects: readonly string[];
  /** The custom fields, one per query name; none when the workspace has none. */
  custom?: readonly CustomQueryField[];
  /** Today, `YYYY-MM-DD`. */
  today: string;
  /** A task's recorded changes, oldest first, for `was:`, `changed:` and `changed-by:`; none when left out. */
  history?: (key: string) => readonly TaskHistory[];
}

/** One recorded change of a task, its values as names (statuses, types, labels and versions by name). */
export interface TaskHistory {
  /** The day it happened, `YYYY-MM-DD`. */
  day: string;
  agent: boolean;
  actor: string;
  /** The task's creation. */
  created: boolean;
  /** The changed fields as the engine's history names them (`due_date`), each value before and after. */
  fields: Readonly<Record<string, { before?: unknown; after?: unknown }>>;
  /** The category of a status named in `fields.status`, by its name in lower case. */
  categories?: Readonly<Record<string, string>>;
}

/** A custom field as queries see it. */
export interface CustomQueryField {
  queryName: string;
  kind: CustomFieldKind;
  options: readonly string[];
}

type FieldType = "custom" | "key" | "project" | "text" | "status" | "category" | "type" | "priority" | "label" | "version" | "parent" | "date" | "span" | "duration" | "number" | "is" | "was" | "changed" | "changed-by";

interface FieldSpec {
  name: string;
  aliases: string[];
  type: FieldType;
  operators: string[];
  sortable: boolean;
  values: string;
  choices?: string[];
  custom?: CustomQueryField;
  /** A condition on the history (`was:status`): which, and the field it looks at. */
  history?: { kind: "was" | "changed"; inner: FieldSpec };
}

/** The history's names for the fields `was:` and `changed:` look at. */
const HISTORY_KEYS: ReadonlyMap<string, string> = new Map([
  ["status", "status"],
  ["category", "status"],
  ["type", "type"],
  ["priority", "priority"],
  ["label", "labels"],
  ["version", "fix_version"],
  ["title", "title"],
  ["start", "start_date"],
  ["due", "due_date"],
  ["estimate", "estimate"],
  ["parent", "parent"],
]);
const WAS_FIELDS = ["status", "category", "type", "priority", "label", "version"];
const CHANGED_FIELDS = ["status", "type", "priority", "title", "start", "due", "estimate", "parent", "label", "version"];

const NAME_OPS = [":", "!="];
const ORDERED_OPS = [":", "!=", "<", "<=", ">", ">="];
const PRIORITIES = ["none", "low", "normal", "high", "urgent"];
const CATEGORIES = ["todo", "in_progress", "done", "cancelled", "open", "closed"];

/** The fields the sample workspace knows: the engine's most used ones. */
export const FAKE_FIELDS: readonly FieldSpec[] = [
  { name: "key", aliases: [], type: "key", operators: NAME_OPS, sortable: true, values: "a task key such as TM-41" },
  { name: "project", aliases: [], type: "project", operators: NAME_OPS, sortable: true, values: "a project key" },
  { name: "title", aliases: [], type: "text", operators: [":"], sortable: true, values: "words in the title" },
  { name: "text", aliases: [], type: "text", operators: [":"], sortable: false, values: "words in the title or description" },
  { name: "status", aliases: [], type: "status", operators: NAME_OPS, sortable: true, values: "status names" },
  { name: "category", aliases: ["cat"], type: "category", operators: NAME_OPS, sortable: true, values: "todo, in_progress, done, cancelled; open, closed", choices: CATEGORIES },
  { name: "type", aliases: [], type: "type", operators: NAME_OPS, sortable: true, values: "task type names" },
  { name: "priority", aliases: ["prio"], type: "priority", operators: ORDERED_OPS, sortable: true, values: "none < low < normal < high < urgent", choices: PRIORITIES },
  { name: "label", aliases: ["labels"], type: "label", operators: NAME_OPS, sortable: false, values: "label names" },
  { name: "version", aliases: ["fix-version"], type: "version", operators: NAME_OPS, sortable: true, values: "version names, or none" },
  { name: "parent", aliases: [], type: "parent", operators: NAME_OPS, sortable: true, values: "a task key, or none" },
  { name: "start", aliases: [], type: "date", operators: ORDERED_OPS, sortable: true, values: "a date, today, +7d, or none" },
  { name: "due", aliases: [], type: "date", operators: ORDERED_OPS, sortable: true, values: "a date, today, +7d, or none" },
  { name: "during", aliases: [], type: "span", operators: [":"], sortable: false, values: "a date, or a range such as 2026-11-01..2026-11-30: start to due overlaps it" },
  { name: "created", aliases: [], type: "date", operators: ORDERED_OPS, sortable: true, values: "a date, today, -7d" },
  { name: "updated", aliases: [], type: "date", operators: ORDERED_OPS, sortable: true, values: "a date, today, -7d" },
  { name: "estimate", aliases: ["est"], type: "duration", operators: ORDERED_OPS, sortable: true, values: "a duration such as 2h, or none" },
  { name: "spent", aliases: [], type: "duration", operators: ORDERED_OPS, sortable: true, values: "a duration such as 2h" },
  { name: "progress", aliases: [], type: "number", operators: ORDERED_OPS, sortable: true, values: "a percentage, 0 to 100" },
  { name: "is", aliases: [], type: "is", operators: [":"], sortable: false, values: "open, closed, blocked", choices: ["open", "closed", "blocked"] },
  { name: "was", aliases: [], type: "was", operators: [":"], sortable: false, values: "a field and a value the task had at any time: was:status:Review", choices: WAS_FIELDS },
  { name: "changed", aliases: [], type: "changed", operators: [":"], sortable: false, values: "a field that changed; with dates, when: changed:due>-7d", choices: CHANGED_FIELDS },
  { name: "changed-by", aliases: [], type: "changed-by", operators: NAME_OPS, sortable: false, values: "me, agent, or an agent's name: who changed the task's fields" },
];

function findField(name: string, names?: QueryNames): FieldSpec | undefined {
  const lower = name.toLowerCase();
  const prefixed = lower.startsWith("cf.");
  const builtin = prefixed ? undefined : FAKE_FIELDS.find((field) => field.name === lower || field.aliases.includes(lower));
  if (builtin) {
    return builtin;
  }
  const custom = names?.custom?.find((field) => field.queryName === (prefixed ? lower.slice(3) : lower));
  return custom ? customSpec(custom) : undefined;
}

function customSpec(custom: CustomQueryField): FieldSpec {
  const ordered = ["integer", "decimal", "date"].includes(custom.kind);
  return {
    name: `cf.${custom.queryName}`,
    aliases: [],
    type: "custom",
    operators: ordered ? ORDERED_OPS : NAME_OPS,
    sortable: true,
    values: custom.kind,
    ...(custom.options.length > 0 ? { choices: [...custom.options] } : custom.kind === "boolean" ? { choices: ["yes", "no"] } : {}),
    custom,
  };
}

/** The registry as `query.parse` returns it. */
export function fieldRegistry(names?: QueryNames): QueryField[] {
  return [...FAKE_FIELDS, ...(names?.custom ?? []).map(customSpec)].map((field) => ({
    name: field.name,
    ...(field.aliases.length > 0 ? { aliases: field.aliases } : {}),
    ...(field.choices ? { choices: field.choices } : {}),
    operators: field.operators,
    ranges: field.type === "span",
    sortable: field.sortable,
    values: field.values,
  }));
}

// ---------- reading ----------

type Node =
  | { kind: "and" | "or"; items: Node[] }
  | { kind: "not"; item: Node }
  | { kind: "cond"; field: FieldSpec; op: string; values: string[]; range: TextRange; valueRanges: TextRange[] }
  | { kind: "text"; words: string[]; ranges: TextRange[] };

class QueryFailure extends Error {
  readonly problem: QueryProblem;
  constructor(messageId: string, message: string, range: TextRange, args: Record<string, string> = {}) {
    super(message);
    this.problem = { message_id: messageId, message, range, ...(Object.keys(args).length > 0 ? { args } : {}) };
  }
}

/** Edit distance, for "Did you mean". */
function distance(a: string, b: string): number {
  const row = Array.from({ length: b.length + 1 }, (_, index) => index);
  for (let i = 1; i <= a.length; i += 1) {
    let previous = row[0] ?? 0;
    row[0] = i;
    for (let j = 1; j <= b.length; j += 1) {
      const kept = row[j] ?? 0;
      row[j] = Math.min((row[j] ?? 0) + 1, (row[j - 1] ?? 0) + 1, previous + (a[i - 1] === b[j - 1] ? 0 : 1));
      previous = kept;
    }
  }
  return row[b.length] ?? 0;
}

function closest(word: string, candidates: readonly string[]): string | undefined {
  let best: string | undefined;
  let bestDistance = 3;
  for (const candidate of candidates) {
    const d = distance(word.toLowerCase(), candidate.toLowerCase());
    if (d < bestDistance) {
      best = candidate;
      bestDistance = d;
    }
  }
  return best;
}

const KEY = /^[A-Z][A-Z0-9]{1,9}-\d+$/;
const DATE = /^(\d{4}-\d{2}-\d{2}|today|yesterday|tomorrow|none|[+-]\d+[dwmy])$/;
const OPERATOR = /^(!=|<=|>=|:|<|>|=)/;

interface Piece {
  kind: "word" | "phrase" | "open" | "close";
  text: string;
  from: number;
  to: number;
}

/** Splits the text into words, quoted phrases and parentheses. */
function pieces(text: string): Piece[] {
  const out: Piece[] = [];
  let at = 0;
  while (at < text.length) {
    const char = text.charAt(at);
    if (/\s/.test(char)) {
      at += 1;
    } else if (char === "(" || char === ")") {
      out.push({ kind: char === "(" ? "open" : "close", text: char, from: at, to: at + 1 });
      at += 1;
    } else {
      // A word runs to a space or a parenthesis outside quotes.
      const from = at;
      let quoted = false;
      let quoteAt = -1;
      while (at < text.length) {
        const c = text.charAt(at);
        if (c === '"') {
          quoted = !quoted;
          quoteAt = at;
        } else if (!quoted && (/\s/.test(c) || c === "(" || c === ")")) {
          break;
        }
        at += 1;
      }
      if (quoted) {
        throw new QueryFailure("query-unclosed-quote", "The quote that starts here is never closed.", { from: quoteAt, to: text.length });
      }
      const word = text.slice(from, at);
      const phrase = word.startsWith('"') && word.endsWith('"') && word.length >= 2;
      out.push({ kind: phrase ? "phrase" : "word", text: word, from, to: at });
    }
  }
  return out;
}

interface Reading {
  tokens: QueryToken[];
  root: Node | null;
  sort: string[];
  advanced: boolean;
}

function token(tokens: QueryToken[], kind: QueryTokenKind, from: number, to: number): void {
  tokens.push({ kind, range: { from, to } });
}

/** Reads one `field op values` word, or a bare word. */
function condition(piece: Piece, tokens: QueryToken[], names: QueryNames): Node | { sort: string[] } {
  const negated = piece.text.startsWith("-") && piece.text.length > 1;
  const start = piece.from + (negated ? 1 : 0);
  const body = piece.text.slice(negated ? 1 : 0);
  if (negated) {
    token(tokens, "negation", piece.from, start);
  }
  const fieldMatch = /^([cC][fF]\.[\p{L}\p{N}_-]+|[a-zA-Z][a-zA-Z-]*)(?=!=|<=|>=|:|<|>|=)/u.exec(body);
  if (piece.kind === "word" && fieldMatch) {
    const name = fieldMatch[1] ?? "";
    const rest = body.slice(name.length);
    const op = OPERATOR.exec(rest)?.[1] ?? ":";
    const valueText = rest.slice(op.length);
    const fieldEnd = start + name.length;
    const valueStart = fieldEnd + op.length;
    if (name.toLowerCase() === "sort") {
      token(tokens, "sort", start, piece.to);
      if (valueText === "") {
        throw new QueryFailure("query-sort-empty", "sort: needs a field, such as sort:due.", { from: start, to: piece.to });
      }
      const keys = valueText.split(",");
      for (const key of keys) {
        const field = findField(key.replace(/^-/, ""), names);
        if (!field?.sortable) {
          const sortable = FAKE_FIELDS.filter((candidate) => candidate.sortable).map((candidate) => candidate.name);
          throw new QueryFailure("query-sort-unknown-plain", `Can't sort by "${key}". Sortable fields: ${sortable.join(", ")}.`, { from: valueStart, to: piece.to }, { field: key, choices: sortable.join(", ") });
        }
      }
      return { sort: keys.map((key) => (key.startsWith("-") ? `-${findField(key.slice(1), names)?.name ?? key}` : (findField(key, names)?.name ?? key))) };
    }
    // A condition on the history names a field of its own: `was:status:Review`.
    const inner = /^([a-zA-Z][a-zA-Z-]*)(!=|<=|>=|:|<|>|=)(.*)$/u.exec(valueText);
    if (op === ":" && (name.toLowerCase() === "was" || name.toLowerCase() === "changed") && inner) {
      return historyCondition(piece, tokens, names, { negated, start, name, fieldEnd, valueStart, inner: inner[1] ?? "", op: inner[2] ?? ":", rest: inner[3] ?? "" });
    }
    const field = findField(name, names);
    if (!field) {
      token(tokens, "error", start, fieldEnd);
      const suggestion = closest(name, FAKE_FIELDS.map((candidate) => candidate.name));
      const range = { from: start, to: fieldEnd };
      throw suggestion
        ? new QueryFailure("query-unknown-field", `Unknown field "${name}". Did you mean "${suggestion}"?`, range, { field: name, suggestion })
        : new QueryFailure("query-unknown-field-plain", `Unknown field "${name}". To search for text with a colon, put it in quotes.`, range, { field: name });
    }
    token(tokens, "field", start, fieldEnd);
    token(tokens, "operator", fieldEnd, valueStart);
    const normalOp = op === "=" ? ":" : op;
    if (!field.operators.includes(normalOp)) {
      throw new QueryFailure("query-operator", `"${op}" doesn't apply to ${field.name}. Use ${field.name}:value.`, { from: fieldEnd, to: valueStart }, { op, field: field.name, usage: `${field.name}:value` });
    }
    if (valueText === "") {
      throw new QueryFailure("query-missing-value", `"${name}${op}" needs a value.`, { from: start, to: piece.to }, { field: name, op });
    }
    const values: string[] = [];
    const valueRanges: TextRange[] = [];
    let at = valueStart;
    for (const raw of valueText.split(",")) {
      const range = { from: at, to: at + raw.length };
      const value = raw.replace(/^"(.*)"$/, "$1");
      if (value !== "") {
        checkValue(field, value, range, names);
        token(tokens, "value", range.from, range.to);
        values.push(value);
        valueRanges.push(range);
      }
      at += raw.length;
      if (at < piece.to) {
        token(tokens, "comma", at, at + 1);
        at += 1;
      }
    }
    const node: Node = { kind: "cond", field, op: normalOp, values, range: { from: piece.from, to: piece.to }, valueRanges };
    return negated ? { kind: "not", item: node } : node;
  }
  if (piece.kind === "word" && KEY.test(body)) {
    token(tokens, "key", start, piece.to);
    const field = findField("key");
    if (!field) {
      throw new Error("the key field is missing");
    }
    const node: Node = { kind: "cond", field, op: ":", values: [body], range: { from: piece.from, to: piece.to }, valueRanges: [{ from: start, to: piece.to }] };
    return negated ? { kind: "not", item: node } : node;
  }
  token(tokens, "text", start, piece.to);
  const node: Node = { kind: "text", words: [body.replace(/^"(.*)"$/, "$1")], ranges: [{ from: start, to: piece.to }] };
  return negated ? { kind: "not", item: node } : node;
}

/** `was:<field><op>values` or `changed:<field><op>dates`. */
function historyCondition(
  piece: Piece,
  tokens: QueryToken[],
  names: QueryNames,
  at: { negated: boolean; start: number; name: string; fieldEnd: number; valueStart: number; inner: string; op: string; rest: string },
): Node {
  const kind = at.name.toLowerCase() === "was" ? "was" : "changed";
  const innerStart = at.valueStart;
  const innerEnd = innerStart + at.inner.length;
  token(tokens, "field", at.start, at.fieldEnd);
  token(tokens, "operator", at.fieldEnd, at.valueStart);
  const allowed = kind === "was" ? WAS_FIELDS : CHANGED_FIELDS;
  const inner = findField(at.inner);
  if (!inner || !allowed.includes(inner.name)) {
    const suggestion = closest(at.inner, allowed);
    const range = { from: innerStart, to: innerEnd };
    throw suggestion
      ? new QueryFailure("query-history-field-suggest", `${kind}: doesn't look up "${at.inner}". Did you mean "${suggestion}"?`, range, { field: at.inner, history: kind, suggestion })
      : new QueryFailure("query-history-field", `${kind}: doesn't look up "${at.inner}". Use one of: ${allowed.join(", ")}.`, range, { field: at.inner, history: kind, choices: allowed.join(", ") });
  }
  token(tokens, "field", innerStart, innerEnd);
  const opEnd = innerEnd + at.op.length;
  token(tokens, "operator", innerEnd, opEnd);
  const op = at.op === "=" ? ":" : at.op;
  const ops = kind === "was" ? NAME_OPS : ORDERED_OPS;
  const written = `${kind}:${inner.name}`;
  if (!ops.includes(op)) {
    throw new QueryFailure("query-operator", `"${at.op}" doesn't apply to ${written}. Use ${written}:value.`, { from: innerEnd, to: opEnd }, { op: at.op, field: written, usage: `${written}:value` });
  }
  if (at.rest === "") {
    throw new QueryFailure("query-missing-value", `"${written}${at.op}" needs a value.`, { from: at.start, to: piece.to }, { field: written, op: at.op });
  }
  const field: FieldSpec = { name: written, aliases: [], type: kind, operators: ops, sortable: false, values: "", history: { kind, inner } };
  const values: string[] = [];
  const valueRanges: TextRange[] = [];
  let from = opEnd;
  for (const raw of at.rest.split(",")) {
    const range = { from, to: from + raw.length };
    const value = raw.replace(/^"(.*)"$/, "$1");
    if (value !== "") {
      if (value.toLowerCase() === "none") {
        throw new QueryFailure("query-history-none", `"none" can't be looked up in the history. For tasks without a ${inner.name} now, use ${inner.name}:none.`, range, { history: kind, field: inner.name });
      }
      checkValue(kind === "was" ? inner : { ...inner, type: "date" }, value, range, names);
      token(tokens, "value", range.from, range.to);
      values.push(value);
      valueRanges.push(range);
    }
    from += raw.length;
    if (from < piece.to) {
      token(tokens, "comma", from, from + 1);
      from += 1;
    }
  }
  const node: Node = { kind: "cond", field, op, values, range: { from: piece.from, to: piece.to }, valueRanges };
  return at.negated ? { kind: "not", item: node } : node;
}

function unknownName(kind: "status" | "type" | "label" | "project", value: string, range: TextRange, known: readonly string[]): QueryFailure {
  const plural = kind === "status" ? "statuses" : `${kind}s`;
  if (known.length === 0) {
    return new QueryFailure(`query-unknown-${kind}-none`, `No ${kind} "${value}": the workspace has no ${plural} yet.`, range, { value });
  }
  return new QueryFailure(`query-unknown-${kind}`, `No ${kind} "${value}". ${plural.charAt(0).toUpperCase()}${plural.slice(1)}: ${known.join(", ")}.`, range, { value, choices: known.join(", ") });
}

function checkValue(field: FieldSpec, value: string, range: TextRange, names: QueryNames): void {
  const has = (list: readonly string[]) => list.some((name) => name.toLowerCase() === value.toLowerCase());
  switch (field.type) {
    case "status":
      if (!has(names.statuses)) {
        throw unknownName("status", value, range, names.statuses);
      }
      return;
    case "type":
      if (!has(names.types)) {
        throw unknownName("type", value, range, names.types);
      }
      return;
    case "label":
      if (value !== "none" && !has(names.labels)) {
        throw unknownName("label", value, range, names.labels);
      }
      return;
    case "project":
      if (!has(names.projects) && !has(names.projects.map((key) => `${key}/*`))) {
        throw unknownName("project", value, range, names.projects);
      }
      return;
    case "priority":
    case "category":
    case "is": {
      const choices = field.choices ?? [];
      if (!choices.includes(value.toLowerCase())) {
        const suggestion = closest(value, choices);
        throw suggestion
          ? new QueryFailure("query-bad-choice-suggest", `"${value}" is not a value of ${field.name}. Did you mean "${suggestion}"?`, range, { value, field: field.name, suggestion })
          : new QueryFailure("query-bad-choice", `"${value}" is not a value of ${field.name}. Use one of: ${choices.join(", ")}.`, range, { value, field: field.name, choices: choices.join(", ") });
      }
      return;
    }
    case "date":
      if (!DATE.test(value)) {
        throw new QueryFailure("query-bad-date", "Dates are written 2026-12-31, or as today, +7d, this-week.", range);
      }
      return;
    case "duration":
      if (value !== "none" && parseDuration(value) === null) {
        throw new QueryFailure("query-bad-duration", `"${value}" is not a duration. Write it as 90m, 1.5h, 1h30m or 2d.`, range, { value });
      }
      return;
    case "number":
      if (!/^\d+$/.test(value) || Number(value) > 100) {
        throw new QueryFailure("query-bad-percent", `"${value}" is not a percentage from 0 to 100.`, range, { value });
      }
      return;
    case "parent":
    case "key":
      if (value !== "none" && !KEY.test(value)) {
        throw new QueryFailure("query-bad-key", `"${value}" is not a task key such as TM-41.`, range, { value });
      }
      return;
    case "text":
    case "changed-by":
      return;
    case "was":
    case "changed": {
      if (field.type === "changed" && !CHANGED_FIELDS.includes(findField(value)?.name ?? "")) {
        throw new QueryFailure("query-history-field", `changed: doesn't look up "${value}". Use one of: ${CHANGED_FIELDS.join(", ")}.`, range, { field: value, history: "changed", choices: CHANGED_FIELDS.join(", ") });
      }
      if (field.type === "was") {
        throw new QueryFailure("query-was-value", "was: needs a field and a value, such as was:status:Review.", range);
      }
      return;
    }
    case "custom": {
      const kind = field.custom?.kind;
      if (value.toLowerCase() === "none") {
        return;
      }
      if ((kind === "select" || kind === "multi_select" || kind === "boolean") && !(field.choices ?? []).some((choice) => choice.toLowerCase() === value.toLowerCase())) {
        const choices = field.choices ?? [];
        throw new QueryFailure("query-bad-choice", `"${value}" is not a value of ${field.name}. Use one of: ${choices.join(", ")}.`, range, { value, field: field.name, choices: choices.join(", ") });
      }
      if ((kind === "integer" || kind === "decimal") && !Number.isFinite(Number(value.split("..")[0]))) {
        throw new QueryFailure("query-bad-number", `"${value}" is not a whole number.`, range, { value });
      }
      return;
    }
  }
}

/** Reads a query: its tokens, its tree, its sort keys. Throws the first mistake. */
function read(text: string, names: QueryNames, tokens: QueryToken[]): Reading {
  const list = pieces(text);
  let index = 0;
  let sort: string[] = [];
  let advanced = false;
  let depth = 0;

  const peek = () => list[index];

  function primary(): Node | null {
    const piece = peek();
    if (!piece) {
      return null;
    }
    if (piece.kind === "open") {
      advanced = true;
      index += 1;
      token(tokens, "paren", piece.from, piece.to);
      depth += 1;
      const inner = orExpression();
      const close = peek();
      if (close?.kind !== "close") {
        throw new QueryFailure("query-unclosed-paren", 'The "(" here has no matching ")".', { from: piece.from, to: piece.to });
      }
      depth -= 1;
      index += 1;
      token(tokens, "paren", close.from, close.to);
      if (!inner) {
        throw new QueryFailure("query-empty-group", "There is nothing inside the parentheses.", { from: piece.from, to: close.to });
      }
      return inner;
    }
    if (piece.kind === "close") {
      if (depth === 0) {
        throw new QueryFailure("query-unexpected-paren", 'There is no "(" for this ")".', { from: piece.from, to: piece.to });
      }
      return null;
    }
    if (piece.kind === "word" && (piece.text === "OR" || piece.text === "AND")) {
      return null;
    }
    index += 1;
    const read = condition(piece, tokens, names);
    if ("sort" in read) {
      if (depth > 0 || sort.length > 0) {
        throw new QueryFailure("query-sort-placement", "sort: can only appear once, outside parentheses.", { from: piece.from, to: piece.to });
      }
      sort = read.sort;
      return primary();
    }
    return read;
  }

  function andExpression(): Node | null {
    const items: Node[] = [];
    for (;;) {
      const piece = peek();
      if (piece?.kind === "word" && piece.text === "AND") {
        token(tokens, "keyword", piece.from, piece.to);
        index += 1;
        if (items.length === 0) {
          throw new QueryFailure("query-condition-before", '"AND" needs a condition before it.', { from: piece.from, to: piece.to }, { keyword: "AND" });
        }
        const next = primary();
        if (!next) {
          throw new QueryFailure("query-condition-after", '"AND" needs a condition after it.', { from: piece.from, to: piece.to }, { keyword: "AND" });
        }
        items.push(next);
        continue;
      }
      const next = primary();
      if (!next) {
        break;
      }
      items.push(next);
    }
    if (items.length === 0) {
      return null;
    }
    return items.length === 1 ? (items[0] ?? null) : { kind: "and", items };
  }

  function orExpression(): Node | null {
    const items: Node[] = [];
    const first = andExpression();
    if (first) {
      items.push(first);
    }
    for (;;) {
      const piece = peek();
      if (piece?.kind !== "word" || piece.text !== "OR") {
        break;
      }
      advanced = true;
      token(tokens, "keyword", piece.from, piece.to);
      index += 1;
      if (items.length === 0) {
        throw new QueryFailure("query-condition-before", '"OR" needs a condition before it.', { from: piece.from, to: piece.to }, { keyword: "OR" });
      }
      const next = andExpression();
      if (!next) {
        throw new QueryFailure("query-condition-after", '"OR" needs a condition after it.', { from: piece.from, to: piece.to }, { keyword: "OR" });
      }
      items.push(next);
    }
    if (items.length === 0) {
      return null;
    }
    return items.length === 1 ? (items[0] ?? null) : { kind: "or", items };
  }

  const root = orExpression();
  const left = peek();
  if (left) {
    throw new QueryFailure("query-unexpected-paren", 'There is no "(" for this ")".', { from: left.from, to: left.to });
  }
  return { tokens, root, sort, advanced };
}

/** The chips of a plain AND of conditions; free words join into one text chip. */
function chipsOf(root: Node | null): QueryChip[] {
  const items = root === null ? [] : root.kind === "and" ? root.items : [root];
  const chips: QueryChip[] = [];
  const text: QueryChip = { negated: false, ranges: [], values: [] };
  for (const item of items) {
    const negated = item.kind === "not";
    const inner = item.kind === "not" ? item.item : item;
    if (inner.kind === "cond") {
      chips.push({ field: inner.field.name, negated, op: inner.op, ranges: [inner.range], values: inner.values });
    } else if (inner.kind === "text" && !negated) {
      text.values.push(...inner.words);
      text.ranges.push(...inner.ranges);
    } else if (inner.kind === "text") {
      chips.push({ field: "text", negated, op: ":", ranges: inner.ranges.map((range) => ({ from: range.from - 1, to: range.to })), values: inner.words });
    }
  }
  if (text.values.length > 0) {
    chips.push(text);
  }
  return chips;
}

/** `query.parse`, as the engine answers it. */
export function parseQuery(text: string, names: QueryNames, withFields = false): ParsedQuery {
  const tokens: QueryToken[] = [];
  const fields = withFields ? { fields: fieldRegistry(names) } : {};
  try {
    const reading = read(text, names, tokens);
    return {
      advanced: reading.advanced,
      ...(reading.advanced ? {} : { chips: chipsOf(reading.root) }),
      ...fields,
      normalized: text.trim().replace(/\s+/g, " "),
      ...(reading.sort.length > 0 ? { sort: reading.sort } : {}),
      tokens,
      valid: true,
    };
  } catch (error: unknown) {
    if (error instanceof QueryFailure) {
      return { advanced: false, error: error.problem, ...fields, tokens, valid: false };
    }
    throw error;
  }
}

// ---------- running ----------

function dayOf(value: string, today: string): string | null {
  if (/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return value;
  }
  const shift = { today: 0, yesterday: -1, tomorrow: 1 }[value] ?? null;
  const relative = /^([+-]\d+)([dwmy])$/.exec(value);
  const base = new Date(`${today}T00:00:00Z`);
  if (shift !== null) {
    base.setUTCDate(base.getUTCDate() + shift);
  } else if (relative) {
    const count = Number(relative[1]);
    const unit = relative[2];
    if (unit === "d") {
      base.setUTCDate(base.getUTCDate() + count);
    } else if (unit === "w") {
      base.setUTCDate(base.getUTCDate() + 7 * count);
    } else if (unit === "m") {
      base.setUTCMonth(base.getUTCMonth() + count);
    } else {
      base.setUTCFullYear(base.getUTCFullYear() + count);
    }
  } else {
    return null;
  }
  return base.toISOString().slice(0, 10);
}

function compare(op: string, a: number | string, b: number | string): boolean {
  switch (op) {
    case "<":
      return a < b;
    case "<=":
      return a <= b;
    case ">":
      return a > b;
    case ">=":
      return a >= b;
    case "!=":
      return a !== b;
    default:
      return a === b;
  }
}

const isClosed = (task: ListedTask) => task.status.category === "done" || task.status.category === "cancelled";

function valueOf(task: ListedTask, field: string): number | string | null {
  if (field.startsWith("cf.")) {
    const value: unknown = task.fields?.[field.slice(3)];
    const first: unknown = Array.isArray(value) ? (value as unknown[])[0] : value;
    return typeof first === "boolean" ? Number(first) : typeof first === "number" || typeof first === "string" ? first : null;
  }
  switch (field) {
    case "key":
      return task.key;
    case "project":
      return task.project;
    case "title":
      return task.title.toLowerCase();
    case "status":
      return task.status.name.toLowerCase();
    case "category":
      return task.status.category;
    case "type":
      return task.type.name.toLowerCase();
    case "priority":
      return PRIORITIES.indexOf(task.priority);
    case "parent":
      return task.parent ?? null;
    case "start":
      return task.start_date ?? null;
    case "due":
      return task.due_date ?? null;
    case "created":
      return task.created_at.slice(0, 10);
    case "updated":
      return task.updated_at.slice(0, 10);
    case "estimate":
      return task.estimate_minutes ?? null;
    case "spent":
      return task.spent_minutes;
    case "progress":
      return task.progress_percent;
    default:
      return null;
  }
}

/** Whether a recorded value (a name, or a list of names) is the one asked for. */
function namedIn(recorded: unknown, wanted: string): boolean {
  const list = Array.isArray(recorded) ? (recorded as unknown[]) : [recorded];
  return list.some((each) => typeof each === "string" && each.toLowerCase() === wanted);
}

/** `was:`, `changed:` and `changed-by:` against the task's recorded changes. */
function matchesHistory(task: ListedTask, field: FieldSpec, op: string, value: string, today: string, history: readonly TaskHistory[]): boolean {
  const lower = value.toLowerCase();
  const changes = history.filter((entry) => !entry.created);
  if (field.type === "changed-by") {
    return changes.some((entry) => (lower === "me" ? !entry.agent : lower === "agent" ? entry.agent : entry.actor.toLowerCase() === lower));
  }
  if (field.type === "changed" && field.history === undefined) {
    const key = HISTORY_KEYS.get(findField(value)?.name ?? "") ?? value;
    return changes.some((entry) => key in entry.fields);
  }
  const inner = field.history?.inner;
  if (!inner) {
    return false;
  }
  const key = HISTORY_KEYS.get(inner.name) ?? inner.name;
  if (field.history?.kind === "changed") {
    const day = dayOf(lower, today) ?? lower;
    return changes.some((entry) => key in entry.fields && compare(op === ":" ? "=" : op, entry.day, day));
  }
  // `was:`: the value now, or before or after a recorded change.
  if (matchesValue(task, inner, ":", value, today)) {
    return true;
  }
  return history.some((entry) => {
    const change = entry.fields[key];
    if (change === undefined) {
      return false;
    }
    if (inner.name === "category") {
      const category = (name: unknown) => (typeof name === "string" ? entry.categories?.[name.toLowerCase()] : undefined);
      const wanted = lower === "open" ? ["todo", "in_progress"] : lower === "closed" ? ["done", "cancelled"] : [lower];
      return [category(change.before), category(change.after)].some((each) => each !== undefined && wanted.includes(each));
    }
    return namedIn(change.before, lower) || namedIn(change.after, lower);
  });
}

function matchesValue(task: ListedTask, field: FieldSpec, op: string, value: string, today: string, history: readonly TaskHistory[] = []): boolean {
  const lower = value.toLowerCase();
  switch (field.type) {
    case "was":
    case "changed":
    case "changed-by":
      return matchesHistory(task, field, op, value, today, history);
    case "is":
      return lower === "open" ? !isClosed(task) : lower === "closed" ? isClosed(task) : task.blocked;
    case "label":
      return lower === "none" ? task.labels.length === 0 : task.labels.some((label) => label.name.toLowerCase() === lower);
    case "version":
      return lower === "none" ? task.fix_version === undefined : task.fix_version?.name.toLowerCase() === lower;
    case "category":
      return lower === "open" ? !isClosed(task) : lower === "closed" ? isClosed(task) : task.status.category === lower;
    case "text": {
      const haystack = task.title.toLowerCase();
      return lower.split(/\s+/).every((word) => haystack.includes(word));
    }
    case "span": {
      // The task's days, from its start to its due date, overlap the range.
      const [from = lower, to = from] = lower.split("..");
      const first = dayOf(from, today);
      const last = dayOf(to, today);
      const start = task.start_date ?? task.due_date;
      const end = task.due_date ?? task.start_date;
      return first !== null && last !== null && start !== undefined && end !== undefined && end >= first && start <= last;
    }
    case "project":
      return lower.endsWith("/*") ? task.project.toLowerCase() === lower.slice(0, -2) || task.project.toLowerCase().startsWith(lower.slice(0, -2)) : task.project.toLowerCase() === lower;
    case "custom": {
      const raw = task.fields?.[field.name.slice(3)];
      if (lower === "none") {
        return raw === undefined;
      }
      const kind = field.custom?.kind;
      if (kind === "boolean") {
        return (raw === true) === ["yes", "true", "1", "on"].includes(lower);
      }
      if (raw === undefined) {
        return false;
      }
      const list = (Array.isArray(raw) ? raw : [raw]).map((each) => String(each).toLowerCase());
      if (kind === "integer" || kind === "decimal") {
        const [from = lower, to] = lower.split("..");
        const actual = Number(list[0]);
        return to === undefined ? compare(op === ":" ? "=" : op, actual, Number(from)) : actual >= Number(from) && actual <= Number(to);
      }
      if (kind === "date") {
        return compare(op === ":" ? "=" : op, list[0] ?? "", dayOf(lower, today) ?? lower);
      }
      if (kind === "select" || kind === "multi_select") {
        return list.includes(lower);
      }
      return list.some((each) => each.includes(lower));
    }
    default: {
      const actual = valueOf(task, field.name);
      if (lower === "none") {
        return op === "!=" ? actual !== null : actual === null;
      }
      if (actual === null) {
        return false;
      }
      let wanted: number | string = lower;
      if (field.type === "priority") {
        wanted = PRIORITIES.indexOf(lower);
      } else if (field.type === "date") {
        wanted = dayOf(lower, today) ?? lower;
      } else if (field.type === "duration") {
        wanted = parseDuration(lower) ?? 0;
      } else if (field.type === "number") {
        wanted = Number(lower);
      } else if (field.type === "key" || field.type === "parent") {
        wanted = value;
      }
      return compare(op === ":" ? "=" : op, actual, wanted);
    }
  }
}

function matches(node: Node, task: ListedTask, today: string, history?: QueryNames["history"]): boolean {
  switch (node.kind) {
    case "and":
      return node.items.every((item) => matches(item, task, today, history));
    case "or":
      return node.items.some((item) => matches(item, task, today, history));
    case "not":
      return !matches(node.item, task, today, history);
    case "text": {
      const haystack = task.title.toLowerCase();
      return node.words.every((word) => haystack.includes(word.toLowerCase()));
    }
    case "cond": {
      const recorded = history?.(task.key) ?? [];
      if (node.op === "!=") {
        return !node.values.some((value) => matchesValue(task, node.field, ":", value, today, recorded));
      }
      return node.values.some((value) => matchesValue(task, node.field, node.op, value, today, recorded));
    }
  }
}

/** Whether a query names projects in a positive condition: archived projects' tasks then count. */
function namesProject(node: Node | null): boolean {
  if (node === null) {
    return false;
  }
  switch (node.kind) {
    case "and":
    case "or":
      return node.items.some(namesProject);
    case "not":
      return false;
    case "cond":
      return node.field.type === "project";
    case "text":
      return false;
  }
}

function sortValue(task: ListedTask, key: string, statusOrder: (task: ListedTask) => number): number | string | null {
  if (key === "status") {
    return statusOrder(task);
  }
  if (key === "version") {
    return task.fix_version?.name.toLowerCase() ?? null;
  }
  if (key === "key") {
    return `${task.project}-${task.key.slice(task.key.indexOf("-") + 1).padStart(8, "0")}`;
  }
  return valueOf(task, key);
}

/** The tasks a query matches, in order. */
function findTasks(
  all: readonly ListedTask[],
  input: TaskSearchInput,
  names: QueryNames,
  options: { archived: ReadonlySet<string>; scoped: boolean; statusOrder: (task: ListedTask) => number },
): { found: ListedTask[]; order: string[] } {
  const reading = read(input.query ?? "", names, []);
  const withArchived = options.scoped || namesProject(reading.root);
  const found = all.filter((task) => (withArchived || !options.archived.has(task.project)) && (reading.root === null || matches(reading.root, task, names.today, names.history)));
  const keys = reading.sort.length > 0 ? reading.sort : (input.sort ?? "").split(",").filter((key) => key !== "");
  const order = keys.length > 0 ? keys : ["status", "rank"];
  found.sort((a, b) => {
    for (const key of order) {
      const descending = key.startsWith("-");
      const name = descending ? key.slice(1) : key;
      const x = name === "rank" ? Number(a.rank) : sortValue(a, name, options.statusOrder);
      const y = name === "rank" ? Number(b.rank) : sortValue(b, name, options.statusOrder);
      if (x === y) {
        continue;
      }
      // Empty values go last in both directions.
      if (x === null) {
        return 1;
      }
      if (y === null) {
        return -1;
      }
      const result = x < y ? -1 : 1;
      return descending ? -result : result;
    }
    return a.project.localeCompare(b.project) || Number(a.key.split("-")[1]) - Number(b.key.split("-")[1]);
  });
  return { found, order };
}

/** What the fake needs to group tasks: their status's colour and place, parents' titles, projects' names, the week's first day. */
export interface GroupInfo {
  statusHue: (task: ListedTask) => Hue | undefined;
  titleOf: (key: string) => string | undefined;
  projectName: (key: string) => string | undefined;
  weekStart: (day: string) => string;
}

interface Grouped {
  found: ListedTask[];
  groups: ListGroup[];
}

const PRIORITY_RANK = ["urgent", "high", "normal", "low", "none"];

/**
 * Groups tasks found in order, as the engine does: groups in their order
 * (empty values last), each group's tasks in the order they were found.
 */
function groupTasks(found: readonly ListedTask[], by: ListGrouping, statusOrder: (task: ListedTask) => number, info: GroupInfo): Grouped {
  const keyOf = (task: ListedTask): { value: string | undefined; order: (string | number)[]; title?: string; hue?: Hue } => {
    switch (by) {
      case "status":
        return { value: task.status.name, order: [statusOrder(task), task.status.name.toLowerCase()], ...some("hue", info.statusHue(task)) };
      case "priority":
        return { value: task.priority, order: [PRIORITY_RANK.indexOf(task.priority)] };
      case "label": {
        const first = [...task.labels].sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()))[0];
        return first ? { value: first.name, order: [first.name.toLowerCase()], hue: first.hue } : { value: undefined, order: [] };
      }
      case "parent":
        return task.parent
          ? { value: task.parent, order: [task.parent.split("-")[0] ?? "", Number(task.parent.split("-")[1])], ...some("title", info.titleOf(task.parent)) }
          : { value: undefined, order: [] };
      case "due_week": {
        const week = task.due_date ? info.weekStart(task.due_date) : undefined;
        return { value: week, order: week ? [week] : [] };
      }
      case "project":
        return { value: task.project, order: [task.project], ...some("title", info.projectName(task.project)) };
    }
  };
  const byValue = new Map<string, { group: ListGroup; order: (string | number)[]; tasks: ListedTask[] }>();
  for (const task of found) {
    const key = keyOf(task);
    const id = key.value ?? "\u0000";
    let entry = byValue.get(id);
    if (!entry) {
      entry = { group: { count: 0, ...some("value", key.value), ...some("title", key.title), ...some("hue", key.hue) }, order: key.order, tasks: [] };
      byValue.set(id, entry);
    }
    entry.group.count += 1;
    entry.tasks.push(task);
  }
  const entries = [...byValue.values()].sort((a, b) => {
    if (a.group.value === undefined || b.group.value === undefined) {
      return a.group.value === undefined ? (b.group.value === undefined ? 0 : 1) : -1;
    }
    for (let at = 0; at < Math.max(a.order.length, b.order.length); at += 1) {
      const x = a.order[at] ?? "";
      const y = b.order[at] ?? "";
      if (x !== y) {
        return x < y ? -1 : 1;
      }
    }
    return 0;
  });
  return { found: entries.flatMap((entry) => entry.tasks), groups: entries.map((entry) => entry.group) };
}

/** `task.search` over the given tasks (already scoped to a project, when one was asked for). */
export function searchTasks(
  all: readonly ListedTask[],
  input: TaskSearchInput,
  names: QueryNames,
  options: { archived: ReadonlySet<string>; scoped: boolean; statusOrder: (task: ListedTask) => number; cache?: Map<string, { found: ListedTask[]; order: string[]; groups?: ListGroup[] }>; groupInfo?: GroupInfo },
): TaskSearchResult {
  const text = input.query ?? "";
  // Pages of one query share its matches: long lists ask for many pages.
  const cacheKey = JSON.stringify([text, input.sort ?? "", input.project ?? "", input.include_subprojects ?? false, input.group_by ?? ""]);
  let cached = options.cache?.get(cacheKey);
  if (!cached) {
    const plain = findTasks(all, input, names, options);
    const grouped = input.group_by && options.groupInfo ? groupTasks(plain.found, input.group_by, options.statusOrder, options.groupInfo) : undefined;
    cached = { found: grouped?.found ?? plain.found, order: plain.order, ...some("groups", grouped?.groups) };
  }
  options.cache?.set(cacheKey, cached);
  const { found, order, groups } = cached;
  const offset = input.cursor ? Number(input.cursor) : 0;
  const limit = Math.min(Math.max(input.limit ?? 50, 1), 200);
  const items = found.slice(offset, offset + limit);
  const end = offset + items.length;
  const totals: ListTotals = { count: found.length, open: 0, estimate_minutes: 0, spent_minutes: 0, remaining_minutes: 0 };
  for (const task of found) {
    totals.open += isClosed(task) ? 0 : 1;
    totals.estimate_minutes += task.estimate_minutes ?? 0;
    totals.spent_minutes += task.spent_minutes;
    totals.remaining_minutes += task.remaining_minutes ?? 0;
  }
  return {
    items,
    ...(end < found.length ? { next_cursor: String(end) } : {}),
    query: text.trim().replace(/\s+/g, " "),
    sort: order,
    total: found.length,
    totals,
    ...(groups && groups.length > 0 ? { groups } : {}),
  };
}

/** The keys of the tasks a query matches, for bulk changes. */
export function matchingKeys(all: readonly ListedTask[], text: string, names: QueryNames, archived: ReadonlySet<string>): string[] {
  const reading = read(text, names, []);
  const withArchived = namesProject(reading.root);
  return all
    .filter((task) => (withArchived || !archived.has(task.project)) && (reading.root === null || matches(reading.root, task, names.today, names.history)))
    .map((task) => task.key);
}

/** The engine's error for a query that doesn't read, as `task.search` reports it. */
export function queryError(error: unknown): { message: string; details: Record<string, unknown> } | null {
  if (!(error instanceof QueryFailure)) {
    return null;
  }
  const { problem } = error;
  return {
    message: problem.message,
    details: { message_id: problem.message_id, args: problem.args ?? {}, from: problem.range.from, to: problem.range.to, text: "" },
  };
}

// ---------- saved views ----------

let viewIds = 0;

function sameName(a: string, b: string) {
  return a.trim().toLowerCase() === b.trim().toLowerCase();
}

/** `view.list`: the views of a project, or of the whole workspace, in rank order. */
export function listViews(views: readonly SavedView[], input: ViewListInput): SavedView[] {
  return views
    .filter((view) => input.all === true || (view.project ?? null) === (input.project ?? null))
    .filter((view) => input.kind === null || input.kind === undefined || view.kind === input.kind)
    .sort((a, b) => a.rank.localeCompare(b.rank));
}

function findView(views: readonly SavedView[], ref: string, project: string | null | undefined): SavedView | undefined {
  return views.find((view) => view.id === ref) ?? views.find((view) => sameName(view.name, ref) && (view.project ?? null) === (project ?? null));
}

/** Problems with a view's input, as the engine words them. */
export type ViewProblem = { code: "invalid" | "not_found" | "conflict"; message: string };

/** `view.create`; the new list of views, or the problem. */
export function createView(views: SavedView[], input: ViewCreateInput, now: string): SavedView | ViewProblem {
  const kind = input.kind ?? "list";
  const project = input.project ?? undefined;
  const name = input.name.trim();
  if (name === "") {
    return { code: "invalid", message: "A view needs a name." };
  }
  if (views.some((view) => sameName(view.name, name) && view.project === project && view.kind === kind)) {
    return { code: "conflict", message: `There is already a view named "${name}".` };
  }
  viewIds += 1;
  const view: SavedView = {
    id: `v-${String(viewIds)}`,
    name,
    kind,
    query: input.query,
    is_default: input.is_default === true,
    rank: String(views.length + 1).padStart(6, "0"),
    created_at: now,
    updated_at: now,
    ...(project !== undefined ? { project } : {}),
    ...(input.sort ? { sort: input.sort } : {}),
    ...(input.columns ? { columns: input.columns } : {}),
    ...(input.group_by ? { group_by: input.group_by } : {}),
    ...(input.totals ? { totals: input.totals } : {}),
    ...(input.description ? { description: input.description } : {}),
  };
  if (view.is_default) {
    for (const other of views) {
      if (other.project === project && other.kind === kind) {
        other.is_default = false;
      }
    }
  }
  views.push(view);
  return view;
}

/** `view.update`; the changed view, or the problem. */
export function updateView(views: SavedView[], input: ViewUpdateInput, now: string): SavedView | ViewProblem {
  const view = findView(views, input.view, input.project);
  if (!view) {
    return { code: "not_found", message: `There is no view "${input.view}".` };
  }
  if (input.name !== null && input.name !== undefined) {
    const name = input.name.trim();
    if (views.some((other) => other !== view && sameName(other.name, name) && other.project === view.project && other.kind === view.kind)) {
      return { code: "conflict", message: `There is already a view named "${name}".` };
    }
    view.name = name;
  }
  if (input.query !== null && input.query !== undefined) {
    view.query = input.query;
  }
  if (input.sort !== null && input.sort !== undefined) {
    view.sort = input.sort;
  }
  if (input.columns) {
    view.columns = input.columns;
  }
  if (input.group_by !== null && input.group_by !== undefined) {
    view.group_by = input.group_by;
  }
  if (input.totals) {
    view.totals = input.totals;
  }
  if (input.is_default !== null && input.is_default !== undefined) {
    if (input.is_default) {
      for (const other of views) {
        if (other.project === view.project && other.kind === view.kind) {
          other.is_default = false;
        }
      }
    }
    view.is_default = input.is_default;
  }
  for (const field of input.clear ?? []) {
    // eslint-disable-next-line @typescript-eslint/no-dynamic-delete -- an optional field the input names
    delete view[field];
  }
  view.updated_at = now;
  return view;
}

/** `view.delete`; the deleted view, or the problem. */
export function deleteView(views: SavedView[], input: ViewDeleteInput): SavedView | ViewProblem {
  const view = findView(views, input.view, input.project);
  if (!view) {
    return { code: "not_found", message: `There is no view "${input.view}".` };
  }
  views.splice(views.indexOf(view), 1);
  return view;
}
