; NSIS 安装钩子脚本
; 用于在卸载软件时清理用户数据

; 卸载完成后执行的钩子：删除应用用户数据目录
; app_data_dir 在 Windows 下为 %APPDATA%\<identifier>，即 $APPDATA\com.gebinee.gebinee
!macro NSIS_HOOK_POSTUNINSTALL
  RmDir /r "$APPDATA\com.gebinee.gebinee"
!macroend
