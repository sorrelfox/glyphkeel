// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { X } from "lucide-react";
import { type ReactNode } from "react";
import { Button, Dialog as AriaDialog, Heading, Modal, ModalOverlay } from "react-aria-components";

import { useT } from "../i18n/I18n";

interface DialogProps {
  title: string;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  children: ReactNode;
  /** Buttons, bottom right, the confirming one last. */
  footer?: ReactNode;
}

/**
 * A small window inside the window: its own header bar with a centred title
 * and a close button. `Esc` and a click outside close it.
 */
export function Dialog({ title, isOpen, onOpenChange, children, footer }: DialogProps) {
  const t = useT();
  return (
    <ModalOverlay className="scrim" isOpen={isOpen} onOpenChange={onOpenChange} isDismissable>
      <Modal className="dialogWindow">
        <AriaDialog className="dialog">
          {({ close }) => (
            <>
              <header className="dialogHeader">
                <Heading slot="title" className="dialogTitle">
                  {title}
                </Heading>
                <Button className="win dialogClose" aria-label={t("shortcuts-close")} onPress={close}>
                  <X className="ic s" />
                </Button>
              </header>
              <div className="dialogBody">{children}</div>
              {footer && <footer className="dialogFooter">{footer}</footer>}
            </>
          )}
        </AriaDialog>
      </Modal>
    </ModalOverlay>
  );
}
