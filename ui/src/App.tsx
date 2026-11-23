// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { HeaderBar } from "./shell/HeaderBar";
import styles from "./shell/Shell.module.css";

export function App() {
  return (
    <div className={styles.shell}>
      <HeaderBar title="Glyphkeel" />
      <main className={styles.content} aria-label="Content" />
    </div>
  );
}
