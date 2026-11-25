!macro NSIS_HOOK_POSTINSTALL
  ; Mark this as an installed build; portable copies omit this file.
  FileOpen $0 "$INSTDIR\installed.flag" w
  FileWrite $0 "installed"
  FileClose $0

  ; Force Explorer to rebuild its icon cache after an update.
  Delete "$LOCALAPPDATA\IconCache.db"
  Delete "$LOCALAPPDATA\Microsoft\Windows\Explorer\iconcache_*.db"
  nsExec::ExecToLog 'ie4uinit.exe -show'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\installed.flag"
!macroend
