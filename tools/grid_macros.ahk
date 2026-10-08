#Requires AutoHotkey v2.0
#SingleInstance Off
; GRID-only timed macros, explicitly requested by the owner.
; Positional args: PID ACTION HOLD_MS Event|Input LOG_PATH EXPECTED_EXE
if A_Args.Length != 6 {
    FileAppend("Expected PID ACTION HOLD_MS MODE LOG_PATH EXPECTED_EXE`n", "*")
    ExitApp(2)
}
global GridPid := Integer(A_Args[1])
global Action := A_Args[2]
global HoldMs := Integer(A_Args[3])
global InputMode := A_Args[4]
global LogPath := A_Args[5]
global GridExe := A_Args[6]
global HeldCodes := []
global StopRequested := false
global Busy := false
if HoldMs < 30 || HoldMs > 2000 || (InputMode != "Event" && InputMode != "Input")
    ExitApp(2)
SendMode(InputMode)
SetKeyDelay(-1, -1)
OnExit((*) => ReleaseHeld())

GridActive(*) {
    global GridPid, GridExe
    try {
        return WinGetPID("A") = GridPid && StrLower(WinGetProcessPath("A")) = StrLower(GridExe)
    } catch {
        return false
    }
}
Log(event, action := "", requested := 0, elapsed := 0, detail := "") {
    global LogPath, InputMode, GridPid
    line := FormatTime(A_NowUTC, "yyyy-MM-dd HH:mm:ss") "`t" GridPid "`t" InputMode "`t" event "`t" action "`t" requested "`t" elapsed "`t" StrReplace(StrReplace(detail, "`n", " "), "`t", " ") "`n"
    FileAppend(line, LogPath, "UTF-8")
    FileAppend(line, "*")
}
ReleaseHeld() {
    global HeldCodes
    for code in HeldCodes
        Send("{" code " up}")
    HeldCodes := []
}
Stop(*) {
    global StopRequested
    StopRequested := true
    ReleaseHeld()
}
Pulse(name, milliseconds) {
    global HeldCodes, StopRequested, Busy
    if Busy
        return
    Busy := true
    started := A_TickCount
    codes := []
    try {
        switch name {
            case "menu_down", "brake": codes := ["sc150"]
            case "menu_up", "throttle": codes := ["sc148"]
            case "left": codes := ["sc14B"]
            case "right": codes := ["sc14D"]
            case "confirm": codes := ["sc01C"]
            case "pause": codes := ["sc001"]
            case "throttle_left": codes := ["sc148", "sc14B"]
            case "throttle_right": codes := ["sc148", "sc14D"]
            default: throw Error("Unknown macro")
        }
        if !GridActive()
            throw Error("Verified GRID is not foreground")
        for key in ["Shift", "Ctrl", "Alt", "Up", "Down", "Left", "Right", "Enter", "Escape", "F12"]
            if GetKeyState(key, "P")
                throw Error("Physical control key is held: " key)
        StopRequested := false
        Log("begin", name, milliseconds)
        started := A_TickCount
        for code in codes {
            HeldCodes.Push(code)
            Send("{" code " down}")
        }
        while A_TickCount - started < milliseconds {
            if StopRequested || GetKeyState("F12", "P") || (name != "pause" && GetKeyState("Escape", "P"))
                throw Error("Physical stop requested")
            if !GridActive()
                throw Error("GRID lost foreground")
            Sleep(5)
        }
        ReleaseHeld()
        Log("end", name, milliseconds, A_TickCount - started, "key-up sent")
        return true
    } catch as err {
        ReleaseHeld()
        Log("error", name, milliseconds, A_TickCount - started, err.Message)
        if Action != "serve"
            ExitApp(1)
        return false
    } finally {
        Busy := false
    }
}

SampleDrive(name) {
    ; Invoke only from an observed paused race with Continue selected.
    ; One bounded input batch keeps tool/screenshot gaps out of the hold.
    Log("sample_begin", "paused_" name, 2000)
    if !Pulse("pause", 150) || !Pulse(name, 2000) || !Pulse("pause", 150) {
        Log("sample_cancelled", "paused_" name, 2000)
        return
    }
    Log("sample_end", "paused_" name, 2000, 0, "normal sequence ended with pause key-up")
}

if Action = "inspect" {
    Log("inspect", "", 0, 0, "AutoHotkey " A_AhkVersion "; GRID foreground=" GridActive() "; no input sent")
    ExitApp()
}
if Action = "serve" {
    ; Bound macros are active only for the exact private GRID process.
    HotIf(GridActive)
    Hotkey("F2", (*) => SampleDrive("throttle_right"))
    Hotkey("F3", (*) => Pulse("left", 150))
    Hotkey("F4", (*) => Pulse("right", 150))
    Hotkey("F5", (*) => SampleDrive("throttle"))
    Hotkey("F6", (*) => Pulse("menu_down", 150))
    Hotkey("F7", (*) => Pulse("menu_up", 150))
    Hotkey("F8", (*) => Pulse("confirm", 150))
    Hotkey("F9", (*) => Pulse("throttle", 500))
    Hotkey("F10", (*) => Pulse("brake", 300))
    Hotkey("F11", (*) => Pulse("pause", 150))
    Hotkey("F12", Stop)
    Persistent()
    Log("ready", "serve", 0, 0, "F2 paused 2000ms throttle/right sample; F3 left; F4 right; F5 paused 2000ms throttle sample; F6 down; F7 up; F8 confirm; F9 throttle; F10 brake; F11 pause; F12 stop")
} else {
    Pulse(Action, HoldMs)
    ExitApp()
}
