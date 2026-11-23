// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { runInNewContext } from "node:vm";

import { describe, expect, it } from "vitest";

import { rewriteFrozenAssignments } from "./frozen-prototype";

/** Runs a library's code as a strict module would, with `Object.prototype` frozen as in the app. */
function runFrozen(code: string): unknown {
  const inherited = Object.freeze(Object.create(null, { valueOf: { value: () => 0 }, toString: { value: () => "" }, constructor: { value: Object } }) as object);
  return runInNewContext(`"use strict"; (function () { ${code} })()`, { base: Object.create(inherited) as object });
}

describe("libraries under a frozen Object.prototype", () => {
  const library = "function D(){} D.prototype = Object.create(base); var p = D.prototype; p.valueOf = function(){ return 42 }; p.toString=function(){return 'd'}; D.prototype.constructor = D; return new D();";

  it("fail as written", () => {
    expect(() => runFrozen(library)).toThrow(/read.only/);
  });

  it("work once rewritten, with the same values", () => {
    const rewritten = rewriteFrozenAssignments(library) ?? "";
    const value = runFrozen(rewritten) as { valueOf(): number; toString(): string; constructor: unknown };
    expect(value.valueOf()).toBe(42);
    expect(String(value)).toBe("d");
    expect(value.constructor).toBeTypeOf("function");
  });

  it("leave comparisons and other code alone", () => {
    expect(rewriteFrozenAssignments("if (a.valueOf === b.valueOf) x = typeof c.toString == 'function';")).toBeNull();
    expect(rewriteFrozenAssignments("#!/usr/bin/env node\na.toString = f;")).toMatch(/^#!\/usr\/bin\/env node\nfunction __gkOwn/);
  });
});
