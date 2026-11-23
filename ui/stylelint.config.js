// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Components and screens read the design tokens only: no literal colour,
 * radius, font size or control height outside the two design stylesheets
 * (tokens.css and components.css), which are kept as the design gives them.
 *
 * @type {import("stylelint").Config}
 */
export default {
  extends: ["stylelint-config-standard"],
  ignoreFiles: ["src/styles/tokens.css", "src/styles/components.css", "dist/**"],
  rules: {
    "color-no-hex": true,
    "color-named": "never",
    "function-disallowed-list": ["rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch", "color"],
    "declaration-property-unit-disallowed-list": {
      "font-size": ["px", "pt", "em", "rem"],
      "/^border(-.*)?-radius$/": ["px", "em", "rem"],
      "/^(min-|max-)?height$/": ["px", "em", "rem"],
    },
    // CSS Modules use camelCase class names, as the components import them.
    "selector-class-pattern": "^[a-z][a-zA-Z0-9]*$",
  },
};
