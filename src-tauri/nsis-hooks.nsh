; ---------------------------------------------------------------------------
; Renown 安装 / 卸载钩子
;
; NSIS 寄存器只有 $0..$9 和 $R0..$R9，没有 $R10 及以上。
; ---------------------------------------------------------------------------

!include "LogicLib.nsh"

; ---------------------------------------------------------------------------
; 安装：若用户选了通用目录，自动加一层 \Renown 子目录
; ---------------------------------------------------------------------------
!macro NSIS_HOOK_PREINSTALL
  StrLen $R0 "\${PRODUCTNAME}"
  IntOp $R0 0 - $R0
  StrCpy $R1 "$INSTDIR" "" $R0
  StrCmp $R1 "\${PRODUCTNAME}" +2
    StrCpy $INSTDIR "$INSTDIR\${PRODUCTNAME}"
  SetOutPath $INSTDIR
!macroend

; ---------------------------------------------------------------------------
; 卸载：用户勾选「删除应用程序数据」时，删除 Renown 真正的数据目录
;
; 背景：Tauri 模板的卸载确认页已经自带「删除应用程序数据」复选框，
; 勾选后会删除 %APPDATA%\<BUNDLEID> 和 %LOCALAPPDATA%\<BUNDLEID>。
; 但 Renown 的实际目录是 %APPDATA%\Renown、%LOCALAPPDATA%\Renown\icon-cache
; 和（可能被改过的）笔记目录，与 BUNDLEID（com.renown.desktop）对不上，
; 所以这里补删真正的三处。
;
; 关键：只在此前已经勾选复选框（$DeleteAppDataCheckboxState = 1）时才删，
; 默认不勾选，因此默认行为是「保留数据」。
; ---------------------------------------------------------------------------
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1

    ; 读出笔记目录：默认 Documents\Renown，可能在 config.json 里被改过
    ; 注意：%APPDATA%\Renown 即将被删除，务必先读再删。
    StrCpy $0 "$DOCUMENTS\Renown"

    ${If} ${FileExists} "$APPDATA\Renown\config.json"
      ClearErrors
      FileOpen $1 "$APPDATA\Renown\config.json" r
      ${IfNot} ${Errors}
        FileRead $1 $2
        FileClose $1

        ; 在 $2 中查找 "dataDir"
        StrLen $4 $2
        StrCpy $3 0
        StrCpy $R0 ""
        ${Do}
          IntOp $R1 $3 + 9
          ${If} $R1 > $4
            ${ExitDo}
          ${EndIf}
          StrCpy $R2 $2 9 $3
          ${If} $R2 == '"dataDir"'
            StrCpy $R0 $3
            ${ExitDo}
          ${EndIf}
          IntOp $3 $3 + 1
        ${Loop}

        ${If} $R0 != ""
          IntOp $3 $R0 + 9
          StrCpy $R2 $2 "" $3
          StrLen $4 $R2

          ; 找值的起始引号
          StrCpy $3 0
          StrCpy $R0 ""
          ${Do}
            ${If} $3 >= $4
              ${ExitDo}
            ${EndIf}
            StrCpy $R1 $R2 1 $3
            ${If} $R1 == '"'
              StrCpy $R0 $3
              ${ExitDo}
            ${EndIf}
            IntOp $3 $3 + 1
          ${Loop}

          ${If} $R0 != ""
            IntOp $3 $R0 + 1
            StrCpy $R1 $R2 "" $3
            StrLen $4 $R1

            ; 找值的结束引号
            StrCpy $3 0
            StrCpy $R0 ""
            ${Do}
              ${If} $3 >= $4
                ${ExitDo}
              ${EndIf}
              StrCpy $R2 $R1 1 $3
              ${If} $R2 == '"'
                StrCpy $R0 $3
                ${ExitDo}
              ${EndIf}
              IntOp $3 $3 + 1
            ${Loop}

            ${If} $R0 != ""
              StrCpy $R2 $R1 $R0

              ; 反转义 \\ -> \（用 $R1 累加结果；$3 遍历下标）
              StrCpy $R1 ""
              StrLen $4 $R2
              StrCpy $3 0
              renown_unescape_loop:
                IntCmp $3 $4 renown_unescape_done
                StrCpy $R5 $R2 1 $3
                IntOp $R0 $3 + 1
                StrCpy $R3 $R2 1 $R0
                StrCmp $R5 "\" 0 renown_unescape_one
                StrCmp $R3 "\" 0 renown_unescape_one
                  StrCpy $R1 "$R1\"
                  IntOp $3 $3 + 2
                  Goto renown_unescape_loop
                renown_unescape_one:
                  StrCpy $R1 "$R1$R5"
                  IntOp $3 $3 + 1
                  Goto renown_unescape_loop
              renown_unescape_done:

              ${If} $R1 != ""
                StrCpy $0 "$R1"
              ${EndIf}
            ${EndIf}
          ${EndIf}
        ${EndIf}
      ${EndIf}
    ${EndIf}

    ; 删除配置与图标缓存（笔记删除必须放在最后，因为它可能等于默认值，
    ; 而 $APPDATA\Renown 与它互不影响）
    RMDir /r "$APPDATA\Renown"
    RMDir /r "$LOCALAPPDATA\Renown\icon-cache"
    ${If} $0 != ""
      RMDir /r "$0"
    ${EndIf}
  ${EndIf}
!macroend
