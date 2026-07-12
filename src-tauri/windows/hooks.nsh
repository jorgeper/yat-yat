; SPEC11 §2.5: tauri's NSIS installerHooks — the deleteAppDataOnUninstall
; equivalent. When the user uninstalls via Add/Remove Programs (or the
; in-app Uninstall button, which launches this uninstaller), the app data
; goes too: settings, history, and the downloaded models.
!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$APPDATA\com.yatyat.app"
  RMDir /r "$LOCALAPPDATA\com.yatyat.app"
!macroend
