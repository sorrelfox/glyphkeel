// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useSortable } from "@dnd-kit/sortable";
import { CSS as DndCss } from "@dnd-kit/utilities";
import { GripVertical, MoreHorizontal } from "lucide-react";
import { useId, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover, Separator, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Switch } from "../../components/Switch";
import { CHART_KINDS, clampWidth, COLUMNS, useWidgetValue, widthAfterDrag } from "../../data/dashboards";
import { failureText } from "../../data/hooks";
import { type Widget } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { widgetTitle, WidgetBody } from "./WidgetBody";
import styles from "./Dashboards.module.css";

interface WidgetCardProps {
  dashboard: string;
  widget: Widget;
  /** Its place, 1 being the first, and how many widgets there are. */
  place: number;
  count: number;
  onMove: (widget: Widget, place: number) => void;
  onResize: (widget: Widget, width: number) => void;
  onEdit: (widget: Widget) => void;
  onRemove: (widget: Widget) => void;
  onOpenTask: (key: string) => void;
  onOpenQuery: (query: string) => void;
}

/**
 * One widget in the grid: a card with its title, a handle to drag it to
 * another place (the arrow keys move it too), a menu with the same moves,
 * a grip at its end edge to make it wider or narrower (a slider for the
 * keyboard), and what it shows. Charts also read as a table.
 */
export function WidgetCard({ dashboard, widget, place, count, onMove, onResize, onEdit, onRemove, onOpenTask, onOpenQuery }: WidgetCardProps) {
  const t = useT();
  const { direction } = useLocale();
  const rtl = direction === "rtl";
  const value = useWidgetValue(dashboard, widget.id);
  const title = widgetTitle(t, widget);
  const titleId = useId();
  const [asTable, setAsTable] = useState(false);
  const { listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id: widget.id });
  const cell = useRef<HTMLDivElement | null>(null);
  const [dragWidth, setDragWidth] = useState<number | null>(null);
  const width = dragWidth ?? widget.width;
  const chart = CHART_KINDS.includes(widget.kind);

  // The drag handle: the arrow keys move the widget a place.
  const onHandleKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    const back = event.key === "ArrowUp" || event.key === (rtl ? "ArrowRight" : "ArrowLeft");
    const forward = event.key === "ArrowDown" || event.key === (rtl ? "ArrowLeft" : "ArrowRight");
    if (back && place > 1) {
      event.preventDefault();
      onMove(widget, place - 1);
    } else if (forward && place < count) {
      event.preventDefault();
      onMove(widget, place + 1);
    }
  };

  // The width grip: dragging its edge snaps to whole columns.
  const onGripDown = (event: PointerEvent<HTMLDivElement>) => {
    const grid = cell.current?.parentElement;
    if (!grid) {
      return;
    }
    event.preventDefault();
    const style = getComputedStyle(grid);
    const gap = Number.parseFloat(style.columnGap) || 0;
    const column = (grid.clientWidth - gap * (COLUMNS - 1)) / COLUMNS;
    const startX = event.clientX;
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    let latest = widget.width;
    const move = (e: globalThis.PointerEvent) => {
      const delta = (e.clientX - startX) * (rtl ? -1 : 1);
      latest = widthAfterDrag(widget.width, delta, column, gap);
      setDragWidth(latest);
    };
    const up = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
      target.removeEventListener("pointercancel", up);
      setDragWidth(null);
      if (latest !== widget.width) {
        onResize(widget, latest);
      }
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
    target.addEventListener("pointercancel", up);
  };
  const onGripKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const wider = event.key === "ArrowUp" || event.key === (rtl ? "ArrowLeft" : "ArrowRight");
    const narrower = event.key === "ArrowDown" || event.key === (rtl ? "ArrowRight" : "ArrowLeft");
    const next = event.key === "Home" ? 1 : event.key === "End" ? COLUMNS : wider ? widget.width + 1 : narrower ? widget.width - 1 : null;
    if (next !== null) {
      event.preventDefault();
      const clamped = clampWidth(next);
      if (clamped !== widget.width) {
        onResize(widget, clamped);
      }
    }
  };

  return (
    <div
      ref={(node) => {
        setNodeRef(node);
        cell.current = node;
      }}
      role="listitem"
      className={classes(styles.cell, isDragging && styles.dragging)}
      style={{ "--span": width, transform: DndCss.Translate.toString(transform), transition } as CSSProperties}
      data-widget={widget.kind}
      data-width={width}
    >
    <article className={styles.widget} aria-labelledby={titleId}>
      <header className={styles.widgetHead}>
        <button
          type="button"
          ref={setActivatorNodeRef}
          className={classes("btn flat icon sm", styles.handle)}
          aria-label={t("widget-move-handle", { title })}
          aria-description={t("widget-move-keys")}
          {...listeners}
          // The keyboard moves it with the arrow keys instead of the drag library's.
          onKeyDown={onHandleKey}
          data-widget-handle
        >
          <GripVertical className="ic s" aria-hidden="true" />
        </button>
        <h2 id={titleId} className={classes("grow truncate", styles.widgetTitle)}>
          {title}
        </h2>
        {chart && (
          <Switch isSelected={asTable} onChange={setAsTable}>
            {t("widget-as-table")}
          </Switch>
        )}
        <MenuTrigger>
          <Button className="btn flat icon sm" aria-label={t("widget-menu", { title })}>
            <MoreHorizontal className="ic s" aria-hidden="true" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu
              className="menu"
              onAction={(key) => {
                switch (key) {
                  case "edit":
                    onEdit(widget);
                    break;
                  case "earlier":
                    onMove(widget, place - 1);
                    break;
                  case "later":
                    onMove(widget, place + 1);
                    break;
                  case "wider":
                    onResize(widget, widget.width + 1);
                    break;
                  case "narrower":
                    onResize(widget, widget.width - 1);
                    break;
                  case "remove":
                    onRemove(widget);
                    break;
                }
              }}
            >
              <MenuItem id="edit" className="item">
                {t("widget-edit")}
              </MenuItem>
              <Separator className="menuSeparator" />
              <MenuItem id="earlier" className="item" isDisabled={place <= 1}>
                {t("widget-earlier")}
              </MenuItem>
              <MenuItem id="later" className="item" isDisabled={place >= count}>
                {t("widget-later")}
              </MenuItem>
              <MenuItem id="wider" className="item" isDisabled={widget.width >= COLUMNS}>
                {t("widget-wider")}
              </MenuItem>
              <MenuItem id="narrower" className="item" isDisabled={widget.width <= 1}>
                {t("widget-narrower")}
              </MenuItem>
              <Separator className="menuSeparator" />
              <MenuItem id="remove" className="item">
                {t("widget-remove")}
              </MenuItem>
            </Menu>
          </Popover>
        </MenuTrigger>
      </header>
      <div className={styles.widgetBody} aria-busy={value.isPending}>
        {value.isError ? (
          <p className={styles.problem} role="alert">
            {t("widget-error", { message: failureText(t, value.error) })}
          </p>
        ) : value.data ? (
          <WidgetBody value={value.data} title={title} asTable={asTable} onOpenTask={onOpenTask} onOpenQuery={onOpenQuery} />
        ) : null}
      </div>
      <div
        className={styles.grip}
        role="slider"
        tabIndex={0}
        aria-label={t("widget-width", { title })}
        aria-orientation="horizontal"
        aria-valuemin={1}
        aria-valuemax={COLUMNS}
        aria-valuenow={widget.width}
        aria-valuetext={t("widget-width-value", { width: widget.width })}
        onPointerDown={onGripDown}
        onKeyDown={onGripKey}
        data-widget-grip
      />
    </article>
    </div>
  );
}
