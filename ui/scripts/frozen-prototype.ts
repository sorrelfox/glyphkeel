// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import type { Plugin } from "vite";

/**
 * The app freezes `Object.prototype`, so a script can't change what every
 * object inherits. A library that sets `valueOf` or `toString` on its own
 * prototype with a plain assignment then fails, because in strict code an
 * assignment can't shadow a read-only inherited property. Mermaid and the
 * libraries it brings do this (dayjs, js-yaml, d3, langium and others). This
 * build step makes each such property an own property of the object just
 * before the library assigns it, so the assignment works, and the global
 * prototype stays frozen. The app's own code never assigns these names.
 */
const LIBRARIES = /[\\/]node_modules[\\/]/;
const ASSIGNMENT = /(^|[^\w$.])((?:[A-Za-z_$][\w$]*)(?:\.[A-Za-z_$][\w$]*)*)\.(valueOf|toString|toLocaleString|constructor)\s*=(?!=)/g;
const HELPER =
  "function __gkOwn(o,k){if(o!==null&&(typeof o==='object'||typeof o==='function')&&Object.isExtensible(o)&&!Object.prototype.hasOwnProperty.call(o,k)){Object.defineProperty(o,k,{configurable:true,enumerable:false,writable:true,value:void 0})}return o}\n";

/** Rewrites `x.valueOf = …` to `__gkOwn(x, "valueOf").valueOf = …` in the listed libraries. */
export function rewriteFrozenAssignments(code: string): string | null {
  const rewritten = code.replace(ASSIGNMENT, (_all, before: string, target: string, name: string) => `${before}__gkOwn(${target},"${name}").${name}=`);
  if (rewritten === code) {
    return null;
  }
  // A script's `#!` line must stay first.
  const shebang = /^#!.*\n/.exec(rewritten)?.[0] ?? "";
  return shebang + HELPER + rewritten.slice(shebang.length);
}

export function frozenPrototype(): Plugin {
  return {
    name: "glyphkeel-frozen-prototype",
    enforce: "pre",
    transform(code, id) {
      if (!LIBRARIES.test(id)) {
        return null;
      }
      const rewritten = rewriteFrozenAssignments(code);
      return rewritten === null ? null : { code: rewritten, map: null };
    },
  };
}
