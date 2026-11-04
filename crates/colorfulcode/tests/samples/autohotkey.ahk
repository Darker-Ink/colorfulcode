#Requires AutoHotkey v2.0
#SingleInstance Force
; Window helpers and hotkeys

/*
  Block comment describing
  the script.
*/

global AppName := "Helper"
counter := 0

^!n::Run "notepad.exe"  ; Ctrl+Alt+N opens Notepad
#j::
{
    MsgBox "Hello from " AppName "`nLine two", "Info", 0x40
}

::btw::by the way
:*:@@::user@example.com

F1::ToggleWindow("ahk_exe chrome.exe")

ToggleWindow(title) {
    global counter
    counter += 1
    if WinActive(title) {
        WinMinimize title
    } else if WinExist(title) {
        WinActivate title
    } else {
        ToolTip "Window not found: " title
        SetTimer () => ToolTip(), -2000
    }
    return counter
}

class Greeter {
    __New(name := "world") {
        this.name := name
    }
    Greet() => "Hello, " this.name "!"
}

Done:
ExitApp
Loop, 3
    MsgBox, Pass %A_Index% of 3, it's fine
