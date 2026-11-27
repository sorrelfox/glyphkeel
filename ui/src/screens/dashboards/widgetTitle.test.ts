// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { starter } from "../../data/fake/dashboards";
import { type Widget } from "../../generated/ops";
import { type Translate } from "../../i18n/I18n";
import { createBundle } from "../../i18n/i18n";
import { widgetTitle } from "./WidgetBody";

const bundle = createBundle("en");
const t: Translate = (id, vars) => {
  const message = bundle.getMessage(id);
  return message?.value ? bundle.formatPattern(message.value, vars).replace(/[⁨⁩]/g, "") : id;
};

function widget(extra: Partial<Widget>): Widget {
  return { id: "w-1", kind: "query_count", settings: {}, width: 1, ...extra };
}

describe("a widget's title", () => {
  it("is never stored for the starter widgets, so each language shows its own", () => {
    expect(starter().every((spec) => spec.title === undefined)).toBe(true);
  });

  it("counts the open tasks in the reader's language when none is given", () => {
    expect(widgetTitle(t, widget({ settings: { query: "is:open" } }))).toBe("Open tasks");
    const shown: string[] = [];
    widgetTitle((id) => (shown.push(id), id), widget({ settings: { query: "is:open" } }));
    expect(shown).toEqual(["widget-open-tasks"]);
  });

  it("keeps a title the person gave, even an old stored English default", () => {
    expect(widgetTitle(t, widget({ title: "Open tasks", settings: { query: "is:open" } }))).toBe("Open tasks");
    expect(widgetTitle(t, widget({ title: "Mine", settings: { query: "is:open" } }))).toBe("Mine");
  });

  it("falls back to the kind's own for other queries", () => {
    expect(widgetTitle(t, widget({ settings: { query: "label:ui" } }))).toBe("Matching tasks");
  });
});
