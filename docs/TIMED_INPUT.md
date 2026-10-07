# Timed GRID input experiment

Recorded: 6 October 2026, Europe/Kiev. Native controller compiled; input and timing behavior **not yet tested**.

The subsequent [AutoHotkey macro implementation](MACRO_SETUP.md) has demonstrated menu and driving response. That successful experiment does not validate this separate Rust/SendInput helper.

The owner asked for an alternative program that lets the agent control buttons and press durations, to improve precision over manually driving the reference. This supersedes the earlier Computer Use API-only choice for this specific GRID input experiment. Other apps and security boundaries remain outside this controller's scope.

## Implementation and scope

Independently authored Rust source: `tools/windows/grid_timed_input.rs`. Compiled with the existing pinned Rust 1.99.0/MSVC environment to private `local/timed-input/grid-timed-input.exe`. No driver, third-party input program or new tool was installed.

The controller uses Windows `SendInput` with scan codes, separate key-down/key-up events and an explicit 30–2000 ms hold. Allowed keys are arrows, Enter and Escape. It records requested and observed wall-clock durations. It checks foreground PID, exact private GRID executable path and process creation FILETIME; it does not activate windows or target arbitrary applications. It refuses interfering held control keys and sends key-up on focus loss, stop keys or ordinary cleanup. F12 stops a hold; physical Escape also stops a non-Escape hold. Console termination handling attempts release. Hard process termination cannot guarantee cleanup.

Windows scheduling and game input polling mean a requested wall-clock duration is not yet a simulation-tick measurement. No claim of deterministic driving, exact in-game consumption or successful menu navigation is made merely because the tool compiles. First test should be a short Down press in an observed paused menu, with a screenshot before and after. Only then test driving pulses and record response. Unsuccessful injection must not be remedied by bypassing a Windows integrity/security barrier.

Primary API references: [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput), [KEYBDINPUT scan-code flags](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput), [INPUT ABI](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-input).

## Interrupted first test

Later clarification requests standard macro tools and examples of Codex playing other games. See [online guide research](GAME_CONTROL_RESEARCH.md): a macro runner can supply timed batches; our Rust helper is one possible backend, rather than a prerequisite to adopting a standard tool. No input test was resumed during that research.

Before any native input was sent, refreshed GRID selection/activation through Computer Use. The tool reported that the owner stopped it with physical Escape and instructed stopping app input. No further Computer Use or native input was attempted this turn. The controller therefore remains untested. The recorded private game process is PID 19680, start `2026-10-06T01:04:50.0562006Z`, creation FILETIME `134357222900562006`; these values must be rechecked on resumption. No race trace has started.
