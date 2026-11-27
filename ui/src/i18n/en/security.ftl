# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The lock screen, and Preferences, Security: the workspace's encryption
# and the app lock.

## The lock screen

lock-title = Glyphkeel is locked
lock-about = Enter the passphrase to go on. The workspace's data stays hidden until then.
lock-workspace-title = This workspace is encrypted
lock-workspace-about = Enter its passphrase to open it.
lock-passphrase = Passphrase
lock-unlock = Unlock
lock-open = Open

## Preferences, Security

preferences-page-security = Security
security-encryption = Encryption
security-encryption-lead = An encrypted workspace can't be read without its passphrase. Attached files are not encrypted; for those, and for the strongest protection, use an encrypted disk.
security-encryption-state = Workspace encryption
security-is-encrypted = The workspace is encrypted.
security-is-plain = The workspace is not encrypted.
security-unavailable = This build of Glyphkeel can't encrypt workspaces.
security-encrypt = Encrypt…
security-change = Change passphrase…
security-decrypt = Decrypt…
security-encrypt-submit = Encrypt
security-change-submit = Change passphrase
security-decrypt-submit = Decrypt
security-encrypt-warning = If the passphrase is lost, the workspace can't be opened again. A backup is made first. Close other Glyphkeel commands and agent servers on this workspace before you go on.
security-decrypt-warning = The workspace will be stored unencrypted again. A backup is made first, and the backups stay encrypted.
security-current = Current passphrase
security-new = New passphrase
security-again = The same passphrase again
security-mismatch = The two passphrases differ.
security-cancel = Cancel
security-encrypted = The workspace is encrypted. The backup made before is { $backup }.
security-changed = The passphrase is changed. The backup made before is { $backup }.
security-decrypted = The workspace is no longer encrypted. The backup made before is { $backup }.
security-backups-left-plain = { $count ->
        [one] One older backup could not be encrypted.
       *[other] { $count } older backups could not be encrypted.
    }
security-lock = App lock
security-lock-after = Lock when idle
security-lock-after-about = Lock the app after this long without a key press or a click.
security-lock-needs-passphrase = Set a passphrase for the lock, or encrypt the workspace, to use the lock.
security-lock-never = Never
security-lock-minutes = { $minutes ->
        [one] After one minute
       *[other] After { $minutes } minutes
    }
security-lock-passphrase = Lock passphrase
security-lock-passphrase-about = The passphrase that unlocks the app. An encrypted workspace unlocks with its own passphrase instead.
security-lock-passphrase-set = A passphrase is set.
security-lock-passphrase-set-button = Set…
security-lock-passphrase-change = Change…
security-lock-passphrase-submit = Save
security-lock-passphrase-remove-hint = Leave the new passphrase empty to remove it.
security-lock-now = Lock now
security-lock-now-about = Hide the workspace until the passphrase is entered again.
security-lock-now-button = Lock
