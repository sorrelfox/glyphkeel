#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The Windows installer under Wine: a silent per-user install, the files,
# shortcuts and registry keys it leaves, then a silent uninstall that removes
# the program and keeps the data.
#
# The checks of what the Windows packaging adds (the install folder under
# %LOCALAPPDATA%\Programs, the command line next to the app and on the user's
# PATH, the glyphkeel:// handler, no desktop shortcut) fail when it is
# missing, or are only skipped with GK_WINE_STRICT=0.
#
# Usage: wine-install.sh PATH/TO/Glyphkeel_<version>_x64-setup.exe
# EXPECTED_VERSION, when set, is the version the uninstall entry and the
# installed command line must show.

. "$(dirname "$0")/wine-lib.sh"

installer=$1
[ -f "$installer" ] || { echo "no installer at $installer" >&2; exit 1; }
new_prefix install

uninstall_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Glyphkeel'
protocol_key='HKCU\Software\Classes\glyphkeel'
local_app_data_win=$(win_env LOCALAPPDATA)
local_app_data=$(unix_path "$local_app_data_win")
expected_dir_win="$local_app_data_win\\Programs\\Glyphkeel"
data_dir="$local_app_data/Glyphkeel"

# Wine has no WebView2. Windows 10 and 11 ship it; the installer looks for
# its version in these keys and installs it only when they are empty, so a
# version here stands for a system that has it.
webview2='{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
wine reg add "HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\$webview2" \
  /v pv /t REG_SZ /d 140.0.0.0 /f >/dev/null 2>&1
wine reg add "HKCU\\Software\\Microsoft\\EdgeUpdate\\Clients\\$webview2" \
  /v pv /t REG_SZ /d 140.0.0.0 /f >/dev/null 2>&1

# Data a user already has, which the uninstall must keep.
mkdir -p "$data_dir/workspaces/Personal"
printf 'keep me\n' >"$data_dir/workspaces/Personal/user-data.txt"

# reg_value KEY NAME: one value of a registry key.
reg_value() {
  reg_query "$1" /v "$2" | sed -n "s/^ *$2 *REG_[A-Z_]* *//p" | head -n 1
}

echo "== silent install"
rc=0
timeout 600 wine "$installer" /S || rc=$?
wineserver -w
check "the installer exits with 0 (got $rc)" [ "$rc" -eq 0 ]

echo "== the uninstall entry"
reg_query "$uninstall_key"
display_name=$(reg_value "$uninstall_key" DisplayName)
check "the uninstall entry is in HKCU (per-user install)" [ -n "$display_name" ]
if [ -n "${EXPECTED_VERSION:-}" ]; then
  check "the uninstall entry shows $EXPECTED_VERSION" \
    [ "$(reg_value "$uninstall_key" DisplayVersion)" = "$EXPECTED_VERSION" ]
fi
install_dir_win=$(reg_value "$uninstall_key" InstallLocation | tr -d '"')
uninstaller_win=$(reg_value "$uninstall_key" UninstallString | tr -d '"')
echo "installed in: $install_dir_win"
install_dir=$(unix_path "$install_dir_win")

echo "== the files"
check "glyphkeel-app.exe is installed" [ -f "$install_dir/glyphkeel-app.exe" ]
check "the uninstaller is installed" [ -f "$(unix_path "$uninstaller_win")" ]
find "$install_dir" -maxdepth 2 -type f | sed "s|^$install_dir/|  |"
if [ "$install_dir_win" = "$expected_dir_win" ]; then
  pass "installed in %LOCALAPPDATA%\\Programs\\Glyphkeel, apart from the data"
else
  pending "installed in %LOCALAPPDATA%\\Programs\\Glyphkeel" \
    "the installer uses $install_dir_win"
fi
cli=
if [ -f "$install_dir/glyphkeel.exe" ]; then
  cli="$install_dir/glyphkeel.exe"
  pass "glyphkeel.exe is next to the app"
  version=$(wine "$cli" --version 2>/dev/null | tr -d '\r')
  if [ -n "${EXPECTED_VERSION:-}" ]; then
    check "the installed command line is $EXPECTED_VERSION" [ "$version" = "glyphkeel $EXPECTED_VERSION" ]
  else
    check "the installed command line runs" [ -n "$version" ]
  fi
  check "the installed command line finds the user's workspace folder" \
    sh -c 'wine "$1" project list >/dev/null 2>&1' sh "$cli"
else
  pending "glyphkeel.exe next to the app" "the installer doesn't carry the command line"
fi

echo "== shortcuts"
start_menu=$(unix_path "$(win_env APPDATA)\\Microsoft\\Windows\\Start Menu\\Programs")
desktop=$(unix_path "$(win_env USERPROFILE)\\Desktop")
check "a Start menu shortcut" sh -c 'find "$1" -iname "Glyphkeel*.lnk" | grep -q .' sh "$start_menu"
# The toasts carry the app's ID; Windows shows them for the app whose
# Start menu shortcut has the same ID. Wine may not write it into the file.
app_id=io.github.sorrelfox.Glyphkeel
shortcut=$(find "$start_menu" -iname 'Glyphkeel*.lnk' | head -n 1)
if [ -n "$shortcut" ] && tr -d '\000' <"$shortcut" | grep -aq "$app_id"; then
  pass "the Start menu shortcut carries the app ID $app_id"
else
  skip "the app ID on the Start menu shortcut: not found in the file (Wine may not store it)"
fi
if find "$desktop" -iname 'Glyphkeel*.lnk' 2>/dev/null | grep -q .; then
  pending "no desktop shortcut" "the installer still adds one"
else
  pass "no desktop shortcut"
fi

echo "== PATH and the glyphkeel:// handler"
user_path=$(reg_value 'HKCU\Environment' Path)
echo "user PATH: $user_path"
on_path=0
case ";$user_path;" in
  *";$install_dir_win;"*) on_path=1 ;;
esac
if [ "$on_path" = 1 ]; then
  pass "the install folder is on the user's PATH"
else
  pending "the install folder on the user's PATH" "the installer doesn't add it"
fi
protocol=$(reg_query "$protocol_key" /s)
has_protocol=0
if printf '%s\n' "$protocol" | grep -q 'URL Protocol'; then
  has_protocol=1
  echo "$protocol"
  pass "glyphkeel:// is registered"
  check "glyphkeel:// opens the app" \
    sh -c 'printf "%s\n" "$1" | grep -i "shell\\\\open\\\\command" -A2 | grep -qi "glyphkeel-app.exe"' sh "$protocol"
else
  pending "the glyphkeel:// handler" "the installer doesn't register it"
fi

# Autostart as the app sets it, switched off in Task Manager: both values
# go with the uninstall.
run_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Run'
approved_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run'
wine reg add "$run_key" /v Glyphkeel /t REG_SZ \
  /d "\"$install_dir_win\\glyphkeel-app.exe\" --background" /f >/dev/null 2>&1
wine reg add "$approved_key" /v Glyphkeel /t REG_BINARY /d 030000000000000000000000 /f >/dev/null 2>&1
check "autostart values to remove are set" \
  sh -c '[ -n "$1" ] && [ -n "$2" ]' sh "$(reg_value "$run_key" Glyphkeel)" "$(reg_value "$approved_key" Glyphkeel)"

echo "== silent uninstall"
rc=0
wine "$(unix_path "$uninstaller_win")" /S || rc=$?
# The uninstaller copies itself and goes on in the background; wait for it.
wineserver -w
i=0
while [ -f "$install_dir/glyphkeel-app.exe" ] && [ "$i" -lt 60 ]; do
  sleep 1
  i=$((i + 1))
done
wineserver -w
check "the uninstaller exits with 0 (got $rc)" [ "$rc" -eq 0 ]
check "glyphkeel-app.exe is removed" [ ! -e "$install_dir/glyphkeel-app.exe" ]
if [ -n "$cli" ]; then
  check "glyphkeel.exe is removed" [ ! -e "$cli" ]
fi
check "the uninstall entry is removed" [ -z "$(reg_value "$uninstall_key" DisplayName)" ]
check "the user's data is kept" [ -f "$data_dir/workspaces/Personal/user-data.txt" ]
check "the autostart value is removed" [ -z "$(reg_value "$run_key" Glyphkeel)" ]
check "Task Manager's autostart state is removed" [ -z "$(reg_value "$approved_key" Glyphkeel)" ]
if [ "$on_path" = 1 ]; then
  case ";$(reg_value 'HKCU\Environment' Path);" in
    *";$install_dir_win;"*) fail "the install folder is taken off the PATH" ;;
    *) pass "the install folder is taken off the PATH" ;;
  esac
fi
if [ "$has_protocol" = 1 ]; then
  check "glyphkeel:// is unregistered" sh -c '! wine reg query "$1" >/dev/null 2>&1' sh "$protocol_key"
fi

finish
