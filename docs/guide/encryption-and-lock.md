<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Encryption and the app lock

Glyphkeel can encrypt a workspace's database with a passphrase, and it can
lock its window after you have been away for a while. Both are optional and
off by default.

If you need your data protected at rest today, the strongest choice is still
your system's **disk encryption** (LUKS on Linux, BitLocker on Windows): it
covers everything, including attached files, backups elsewhere on the disk,
thumbnails and the operating system's own caches. Glyphkeel's encryption
adds a passphrase for one workspace on top of that, or protects a workspace
on a disk that isn't encrypted.

## Encrypting a workspace

In the app: **Preferences › Security › Encrypt…**. Type the passphrase twice
and choose **Encrypt**. On the command line:

```sh
glyphkeel workspace encrypt              # asks for the passphrase twice
echo "my passphrase" | glyphkeel workspace encrypt   # or reads one line
```

What happens:

1. A backup is made first.
2. The whole database is written into a new, encrypted file, which is
   checked and then put in place of the old one in one step.
3. The backups in the workspace's backup folder are encrypted too, with the
   same passphrase.

Nobody else may have the workspace open meanwhile: close other Glyphkeel
windows on it, `glyphkeel` commands and agent servers (`glyphkeel mcp`)
first. The app closes and reopens the workspace by itself.

**If you forget the passphrase, the workspace can't be opened again.**
There is no way around it, by design. Keep the passphrase somewhere safe,
such as a password manager.

## Opening an encrypted workspace

- **The app** asks for the passphrase when it starts, before it shows
  anything of the workspace.
- **The command line** asks on the terminal, without showing what you type.
  In scripts, where there is no terminal, it reads the passphrase from the
  environment variable `GLYPHKEEL_PASSPHRASE`, or fails with a message that
  says so.
- **Agent servers** (`glyphkeel mcp`) can't ask: they read
  `GLYPHKEEL_PASSPHRASE`. Put it in the MCP client's configuration of the
  server only if that configuration is as private as the workspace itself.

Glyphkeel never stores the passphrase, and it doesn't use the system's
keyring. See [Security](../security.md#encrypted-workspaces) for the
trade-offs of the environment variable.

## Changing the passphrase, or decrypting

**Preferences › Security › Change passphrase…** or **Decrypt…**, or
`glyphkeel workspace encrypt` (on an encrypted workspace it sets a new
passphrase) and `glyphkeel workspace decrypt`. Both need the current
passphrase and make a backup first.

- After a change of passphrase, older backups keep the passphrase they were
  made with.
- After decrypting, the backups stay encrypted.

## Backups and restoring

Backups of an encrypted workspace are encrypted with its passphrase. To
restore one that was made with an older passphrase, Glyphkeel asks for that
passphrase. A restored workspace keeps the workspace's current state:
restoring an encrypted backup into a plain workspace gives a plain
workspace, and the other way round.

## What isn't encrypted

- **Attached files** (the `files` folder) and their thumbnails.
- **Exports** you make (archives, CSV, Markdown, pictures).
- **The settings files** of the app and the command line.
- **Copies another program keeps**, such as an editor that opened an
  attachment.

## The app lock

The app lock hides the workspace behind a passphrase screen. While the app
is locked:

- no window shows any task, page or file, and the app refuses every request
  for data from its windows;
- reminders still come, but their notifications don't name the task;
- other programs (commands, agent servers) go on working.

It unlocks with the workspace's passphrase when the workspace is encrypted.
For a workspace that isn't, set a **Lock passphrase** in
**Preferences › Security**. The password of your user account is not used.

Lock the app:

- by hand: **Ctrl+Shift+L**, **Lock Glyphkeel** in the command palette
  (Ctrl+K), or **Lock** in **Preferences › Security**;
- by itself: choose **Lock when idle** (one minute to an hour) in
  **Preferences › Security**. Every key press or click in a Glyphkeel window
  starts the time again.

The lock protects against someone at your unlocked computer. It doesn't
protect against programs running as you: the workspace stays open while the
app is locked.

## Which builds can encrypt

The Linux builds (packages, Flatpak, AUR) can. The Windows build can't yet:
it opens plain workspaces only, and its app lock works with a lock
passphrase.
