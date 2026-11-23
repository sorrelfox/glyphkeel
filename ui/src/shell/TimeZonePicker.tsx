// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Check, ChevronDown } from "lucide-react";
import { Button, ComboBox, Input, ListBox, ListBoxItem, Popover, useFilter } from "react-aria-components";

import { classes } from "../classes";
import selectStyles from "../components/Select.module.css";
import { useT } from "../i18n/I18n";
import { SYSTEM_ZONE, type ZoneChoice } from "../ipc/appSettings";
import styles from "./Preferences.module.css";

/** An offset from UTC as "+05:30" or "−08:00". */
export function offsetText(seconds: number): string {
  const minutes = Math.round(Math.abs(seconds) / 60);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${seconds < 0 ? "−" : "+"}${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
}

/** A zone's name as people read it: "America/New York". */
function readable(name: string): string {
  return name.replace(/_/gu, " ");
}

interface Item {
  id: string;
  text: string;
  offset: string | null;
}

interface TimeZonePickerProps {
  label: string;
  /** `"system"` or an IANA name. */
  value: string;
  /** The system's zone by name, shown with "System". */
  systemZone: string | undefined;
  zones: readonly ZoneChoice[];
  onChange: (zone: string) => void;
}

/**
 * The time zone: "System", or one zone from a list that filters as the
 * person types. Each zone shows its offset from UTC now.
 */
export function TimeZonePicker({ label, value, systemZone, zones, onChange }: TimeZonePickerProps) {
  const t = useT();
  const { contains } = useFilter({ sensitivity: "base" });
  const system = systemZone === undefined ? t("preferences-time-zone-system") : t("preferences-time-zone-system-named", { zone: readable(systemZone) });
  const items: Item[] = [
    { id: SYSTEM_ZONE, text: system, offset: null },
    ...zones.map((zone) => ({ id: zone.name, text: readable(zone.name), offset: offsetText(zone.offset_seconds) })),
  ];
  return (
    <ComboBox
      aria-label={label}
      className={classes(selectStyles.select, styles.zone)}
      defaultItems={items}
      value={value}
      defaultFilter={(text, input) => contains(text, input) || contains(text.replace(/ /gu, "_"), input)}
      menuTrigger="focus"
      allowsEmptyCollection
      onChange={(key) => {
        if (typeof key === "string" && key !== value) {
          onChange(key);
        }
      }}
    >
      <div className={classes("field", selectStyles.trigger)}>
        <Input className={classes(styles.zoneInput)} />
        <Button className="btn flat icon sm" aria-label={t("preferences-time-zone-list")}>
          <ChevronDown className={classes("ic s", selectStyles.chevron)} aria-hidden="true" />
        </Button>
      </div>
      <Popover className={classes("pop menuPopover", selectStyles.popover, styles.zonePopover)} placement="bottom end" offset={4}>
        <ListBox className="menu" renderEmptyState={() => <p className={classes("muted", styles.zoneEmpty)}>{t("preferences-time-zone-none")}</p>}>
          {(item: Item) => (
            <ListBoxItem id={item.id} textValue={item.text} className="item">
              {({ isSelected }) => (
                <>
                  <span className="grow truncate">{item.text}</span>
                  {item.offset !== null && <span className="muted num">{t("preferences-time-zone-offset", { offset: item.offset })}</span>}
                  <Check className={classes("ic s", selectStyles.check, !isSelected && selectStyles.hidden)} aria-hidden="true" />
                </>
              )}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </ComboBox>
  );
}
