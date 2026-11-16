; FreeSpace NSIS hooks for the Tauri 2 installer.
;
; Tauri copies the CLI next to the GUI and creates the shortcuts itself. These
; hooks only handle the application-owned state:
;
;   * install   — remove Explorer entries left by an older version and create
;                 the writable `data` folder next to the executable;
;   * uninstall — stop pointing the context menu at the binaries and delete
;                 `data` (settings, journal, quarantine, hash cache, WebView2
;                 profile) so no trace stays outside the application folder.
;
; Missing context-menu entries are ignored by the CLI, so both hooks are
; idempotent.

!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$INSTDIR\data"
  ; A fresh install must contain no Explorer integration, even after an
  ; in-place upgrade from a version that registered it by default.
  IfFileExists "$INSTDIR\freespace.exe" 0 +2
    ExecWait '"$INSTDIR\freespace.exe" context-menu uninstall --all --yes' $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; The binaries still exist here; let the CLI clean both registry scopes.
  IfFileExists "$INSTDIR\freespace.exe" 0 +2
    ExecWait '"$INSTDIR\freespace.exe" context-menu uninstall --all --yes' $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove everything the application wrote. User files outside `$INSTDIR`
  ; are never touched.
  RMDir /r "$INSTDIR\data"
!macroend
