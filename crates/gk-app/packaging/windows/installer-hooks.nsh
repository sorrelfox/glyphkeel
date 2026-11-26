; SPDX-License-Identifier: GPL-3.0-or-later
; Copyright (C) 2026 Glyphkeel contributors
;
; Additions to the Windows installer that the Tauri bundler builds.
;
; - The programs go to %LOCALAPPDATA%\Programs\Glyphkeel, never into
;   %LOCALAPPDATA%\Glyphkeel, which holds the workspaces and backups.
; - `glyphkeel.exe`, the command line, is installed next to the app, and its
;   folder is added to the user's PATH.
; - `glyphkeel://` links open the app.
; - No desktop shortcut unless the user ticks the box on the last page.
; - Uninstalling removes the programs, the PATH entry and the link handler,
;   and keeps every workspace, backup and setting.
;
; The bundler includes this file near the top of its script, before the
; pages are declared, and calls the NSIS_HOOK_* macros from its sections.
; The command line must be built into the same folder as the app before the
; bundler runs.

; This file is read before the bundler defines PRODUCTNAME and
; MAINBINARYNAME, so the functions below use their own names; they must match
; `productName` and `mainBinaryName` in tauri.conf.json.
!define GK_PRODUCT "Glyphkeel"
!define GK_APP_NAME "glyphkeel-app.exe"
!define GK_CLI_NAME "glyphkeel.exe"
!define GK_LINK_SCHEME "glyphkeel"
!define GK_ENV_KEY "Environment"
; The longest string the installer handles, without its terminating zero.
!define /math GK_STRING_LIMIT ${NSIS_MAX_STRLEN} - 1

; The licence is the GPL: nothing to agree to, so the page only shows it.
!define MUI_LICENSEPAGE_BUTTON "$(^NextBtn)"
!define MUI_LICENSEPAGE_TEXT_BOTTOM "$(^ClickNext)"

; The desktop shortcut on the last page is offered, not ticked.
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED

; Move the default folder before the folder page is shown.
!define MUI_CUSTOMFUNCTION_GUIINIT GkGuiInit

; ---------- install folder ----------

; Never install into the data folder: the default (and an earlier install
; there) moves to the Programs folder.
!macro GK_LEAVE_DATA_FOLDER
  ${If} $INSTDIR == "$LOCALAPPDATA\${GK_PRODUCT}"
  ${OrIf} $INSTDIR == "$LOCALAPPDATA\${GK_PRODUCT}\"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\${GK_PRODUCT}"
  ${EndIf}
!macroend

Function GkGuiInit
  !insertmacro GK_LEAVE_DATA_FOLDER
FunctionEnd

; ---------- the user's PATH ----------

; Whether HKCU\Environment has a Path value at all. Out: $R9 = 1 or 0.
Function GkPathExists
  Push $0
  Push $1
  StrCpy $R9 0
  StrCpy $0 0
  loop:
    ClearErrors
    EnumRegValue $1 HKCU "${GK_ENV_KEY}" $0
    IfErrors done
    StrCmp $1 "" done
    ${If} $1 == "Path"
      StrCpy $R9 1
      Goto done
    ${EndIf}
    IntOp $0 $0 + 1
    Goto loop
  done:
  Pop $1
  Pop $0
FunctionEnd

; A PATH without the entries that name a folder; every other entry is kept
; as it was. In: $R0 = PATH, $R1 = folder. Out: $R2.
!macro GK_DEFINE_PATH_WITHOUT UN
Function ${UN}GkPathWithout
  Push $R3 ; the entry being read
  Push $R4 ; the current character
  Push $R5 ; position in $R0
  Push $R6 ; 1 once an entry has been kept
  StrCpy $R2 ""
  StrCpy $R3 ""
  StrCpy $R5 0
  StrCpy $R6 0
  loop:
    StrCpy $R4 $R0 1 $R5
    ${If} $R4 == ";"
    ${OrIf} $R4 == ""
      ; StrCmp, and so ==, ignores case, as Windows does for paths.
      ${If} $R3 != $R1
      ${AndIf} $R3 != "$R1\"
        ${If} $R6 = 1
          StrCpy $R2 "$R2;$R3"
        ${Else}
          StrCpy $R2 $R3
          StrCpy $R6 1
        ${EndIf}
      ${EndIf}
      StrCpy $R3 ""
      StrCmp $R4 "" done
    ${Else}
      StrCpy $R3 "$R3$R4"
    ${EndIf}
    IntOp $R5 $R5 + 1
    Goto loop
  done:
  Pop $R6
  Pop $R5
  Pop $R4
  Pop $R3
FunctionEnd
!macroend

!insertmacro GK_DEFINE_PATH_WITHOUT ""
!insertmacro GK_DEFINE_PATH_WITHOUT "un."

; Tell running programs (Explorer, new terminals) that the PATH changed.
!macro GK_ANNOUNCE_ENVIRONMENT
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

; A PATH value that can't be read in full (longer than the installer's
; strings, or not a string) is left alone rather than cut short. The app's
; Preferences then report that the command line is not on the PATH.
Function GkAddToPath
  ClearErrors
  ReadRegStr $R0 HKCU "${GK_ENV_KEY}" "Path"
  ${If} ${Errors}
    Call GkPathExists
    ${If} $R9 = 1
      DetailPrint "PATH not changed: the current value could not be read in full"
      Return
    ${EndIf}
    StrCpy $R0 ""
  ${EndIf}
  StrLen $R3 $R0
  ${If} $R3 >= ${GK_STRING_LIMIT}
    DetailPrint "PATH not changed: the current value is too long"
    Return
  ${EndIf}

  StrCpy $R1 $INSTDIR
  Call GkPathWithout
  ${If} $R2 != $R0
    ; Already on the PATH.
    Return
  ${EndIf}

  ${If} $R0 == ""
    StrCpy $R2 $INSTDIR
  ${Else}
    StrCpy $R3 $R0 1 -1
    ${If} $R3 == ";"
      StrCpy $R2 "$R0$INSTDIR"
    ${Else}
      StrCpy $R2 "$R0;$INSTDIR"
    ${EndIf}
  ${EndIf}
  StrLen $R3 $R2
  ${If} $R3 >= ${GK_STRING_LIMIT}
    DetailPrint "PATH not changed: the new value would be too long"
    Return
  ${EndIf}
  WriteRegExpandStr HKCU "${GK_ENV_KEY}" "Path" $R2
  !insertmacro GK_ANNOUNCE_ENVIRONMENT
FunctionEnd

Function un.GkRemoveFromPath
  ClearErrors
  ReadRegStr $R0 HKCU "${GK_ENV_KEY}" "Path"
  ${If} ${Errors}
    Return
  ${EndIf}
  StrLen $R3 $R0
  ${If} $R3 >= ${GK_STRING_LIMIT}
    DetailPrint "PATH not changed: the current value is too long"
    Return
  ${EndIf}

  StrCpy $R1 $INSTDIR
  Call un.GkPathWithout
  ${If} $R2 == $R0
    Return
  ${EndIf}
  ${If} $R2 == ""
    DeleteRegValue HKCU "${GK_ENV_KEY}" "Path"
  ${Else}
    WriteRegExpandStr HKCU "${GK_ENV_KEY}" "Path" $R2
  ${EndIf}
  !insertmacro GK_ANNOUNCE_ENVIRONMENT
FunctionEnd

; ---------- glyphkeel:// links ----------

Function GkRegisterLinks
  WriteRegStr HKCU "Software\Classes\${GK_LINK_SCHEME}" "" "URL:${GK_PRODUCT}"
  WriteRegStr HKCU "Software\Classes\${GK_LINK_SCHEME}" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\${GK_LINK_SCHEME}\DefaultIcon" "" "$\"$INSTDIR\${GK_APP_NAME}$\",0"
  WriteRegStr HKCU "Software\Classes\${GK_LINK_SCHEME}\shell\open\command" "" "$\"$INSTDIR\${GK_APP_NAME}$\" $\"%1$\""
FunctionEnd

; Only when the handler is still this install's: another install may own it.
Function un.GkUnregisterLinks
  ReadRegStr $R0 HKCU "Software\Classes\${GK_LINK_SCHEME}\shell\open\command" ""
  ${If} $R0 == "$\"$INSTDIR\${GK_APP_NAME}$\" $\"%1$\""
    DeleteRegKey HKCU "Software\Classes\${GK_LINK_SCHEME}"
  ${EndIf}
FunctionEnd

; ---------- the hooks ----------

!macro NSIS_HOOK_PREINSTALL
  !if "${PRODUCTNAME}" != "${GK_PRODUCT}"
    !error "GK_PRODUCT does not match the product name in tauri.conf.json"
  !endif
  !if "${MAINBINARYNAME}.exe" != "${GK_APP_NAME}"
    !error "GK_APP_NAME does not match the main binary name in tauri.conf.json"
  !endif

  ; A silent install never shows the folder page. The bundler has already
  ; created the folder it chose; the data folder is left as it was found,
  ; which removes it only if it is empty.
  StrCpy $R0 $INSTDIR
  !insertmacro GK_LEAVE_DATA_FOLDER
  ${If} $INSTDIR != $R0
    SetOutPath $INSTDIR
    RMDir $R0
  ${EndIf}

  ; The command line, built next to the app.
  !searchreplace GK_CLI_SOURCE "${MAINBINARYSRCPATH}" "${MAINBINARYNAME}.exe" "${GK_CLI_NAME}"
  !if /FileExists "${GK_CLI_SOURCE}"
  !else
    !error "${GK_CLI_SOURCE} is missing: build the command line for this target and profile before the installer"
  !endif
  File "/oname=${GK_CLI_NAME}" "${GK_CLI_SOURCE}"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Call GkAddToPath
  Call GkRegisterLinks

  ; The bundler adds a desktop shortcut to every silent or passive install.
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; A running command line (an MCP server, say) is removed at the next
  ; sign-in.
  Delete /REBOOTOK "$INSTDIR\${GK_CLI_NAME}"

  ; An update installs over this one and sets them again.
  ${If} $UpdateMode <> 1
    Call un.GkRemoveFromPath
    Call un.GkUnregisterLinks
  ${EndIf}
!macroend

; The data folders are never touched: %LOCALAPPDATA%\Glyphkeel (workspaces,
; backups) and %APPDATA%\Glyphkeel (settings).
