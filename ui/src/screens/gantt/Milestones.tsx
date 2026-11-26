// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Diamond } from "lucide-react";
import { Button, Dialog, DialogTrigger, Heading, Popover, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { useSchedule } from "../../data/schedule";
import { useT } from "../../i18n/I18n";
import { asDate } from "../calendar/model";
import { milestonesOf, type Milestone } from "./model";
import styles from "./Gantt.module.css";

interface MilestonesProps {
  project: string;
  includeSubprojects: boolean;
  /** Shows a milestone in the chart; false when the chart's filter hides it. */
  onShow: (key: string) => boolean;
  /** Opens a milestone's task. */
  onOpen: (key: string) => void;
}

/**
 * The project's milestones in a list beside the chart: each with its date
 * and how it stands (done, overdue, or its status). The list ignores the
 * chart's filter, so every milestone is there; choosing one shows it in the
 * chart, or opens it when the filter hides it.
 */
export function MilestonesButton({ project, includeSubprojects, onShow, onOpen }: MilestonesProps) {
  const t = useT();
  return (
    <DialogTrigger>
      <Button className="btn flat" aria-label={t("gantt-milestones")}>
        <Diamond className="ic" aria-hidden="true" />
        <span className="hide-narrow">{t("gantt-milestones")}</span>
      </Button>
      <Popover className="pop" placement="bottom end" offset={4}>
        <Dialog className={classes(styles.milestones)}>
          {({ close }) => (
            <MilestoneList
              project={project}
              includeSubprojects={includeSubprojects}
              onShow={(key) => {
                close();
                // The chart takes the focus once the popover has given it back.
                requestAnimationFrame(() => {
                  if (!onShow(key)) {
                    onOpen(key);
                  }
                });
              }}
            />
          )}
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}

function MilestoneList({ project, includeSubprojects, onShow }: { project: string; includeSubprojects: boolean; onShow: (key: string) => void }) {
  const t = useT();
  const { locale } = useLocale();
  const data = useSchedule({ project, includeSubprojects, query: "" });
  const full = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });
  const milestones = data.schedule ? milestonesOf(data.schedule) : undefined;

  const stateText = (milestone: Milestone) => {
    // Overdue says by how long; otherwise the status says how it stands.
    return milestone.state === "overdue" ? t("gantt-overdue-by", { days: milestone.late }) : milestone.status;
  };

  return (
    <>
      <Heading slot="title" className={classes(styles.milestonesHeading)}>
        {t("gantt-milestones")}
      </Heading>
      {milestones === undefined ? (
        <p className={styles.milestonesNote} aria-busy="true" />
      ) : milestones.length === 0 ? (
        <p className={styles.milestonesNote}>{t("gantt-milestones-none")}</p>
      ) : (
        <ul className={styles.milestoneList} data-testid="gantt-milestones">
          {milestones.map((milestone) => (
            <li key={milestone.key}>
              <Button
                className={classes(styles.milestone, milestone.state === "done" && styles.milestoneDone)}
                data-milestone={milestone.key}
                data-state={milestone.state}
                onPress={() => {
                  onShow(milestone.key);
                }}
              >
                <Diamond className={classes("ic s", styles.milestoneMark)} aria-hidden="true" />
                <span className={styles.milestoneKey}>{milestone.key}</span>
                <span className={styles.milestoneTitle}>{milestone.title}</span>
                <span className={classes(styles.milestoneDate, "num")}>{milestone.date === undefined ? t("gantt-milestone-no-date") : full.format(asDate(milestone.date))}</span>
                <span className={classes(styles.milestoneState, milestone.state === "overdue" && styles.milestoneLate)}>{stateText(milestone)}</span>
              </Button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
