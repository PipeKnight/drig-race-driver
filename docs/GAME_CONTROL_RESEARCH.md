# Macro tools and Codex game control

Checked online: 6 October 2026. Research only; no macro software installed, game input sent or third-party MCP registered in this session.

Subsequent implementation: [AutoHotkey setup and verified GRID controls](MACRO_SETUP.md). Menu navigation, bound hotkeys, throttle, combined throttle/right and pause now work; the proposals below remain the historical research context.

## Answer for GRID

A macro runner is a valid way to execute button sequences locally with explicit hold durations. Codex can select a named macro, trigger its hotkey or invoke a command, then inspect the result. The local runner performs the timing while the model waits. Our independently authored Rust input experiment is already a possible low-level backend, but has not yet demonstrated GRID input acceptance. Creating a hotkey does not itself establish that the downstream synthetic events reach GRID.

AutoHotkey v2 is a reasonable standard macro tool to test. Its [original Send documentation](https://github.com/AutoHotkey/AutoHotkeyDocs/blob/v2/docs/lib/Send.htm#repeating-or-holding-down-a-key) describes separate down/up events with Sleep between them. [SetKeyDelay](https://github.com/AutoHotkey/AutoHotkeyDocs/blob/v2/docs/lib/SetKeyDelay.htm) documents press duration for specialized applications, including games; that setting applies to SendEvent/SendPlay and is ignored by default SendInput mode. Start with a short menu pulse, compare event/scan-code approaches if needed, and observe the resulting selection before driving.

Example proposed macro behavior: press Up, wait a requested 500 ms, release Up. A combined throttle-and-steering macro can keep Up held while Right is held for a shorter interval. Neither example has been tested in GRID. [Sleep documentation](https://github.com/AutoHotkey/AutoHotkeyDocs/blob/v2/docs/lib/Sleep.htm) explains that requested waits are subject to OS granularity and scheduling; millisecond parameters are not proof of exact simulation ticks.

Inventory before installation: AutoHotkey commands were absent from the inspected PATH. No AutoHotkey executable found under inspected Program Files, LocalAppData Programs or user Tools roots; uninstall registrations matched none of AutoHotkey, Macro Recorder, Pulover or AutoIt. This is a bounded inventory, not proof that no portable copy exists anywhere. Existing Rust/MSVC and our compiled helper remain available.

## What Minecraft guides actually connect

[Mineflayer](https://github.com/PrismarineJS/mineflayer) provides a programmatic Minecraft bot API. The [Minecraft AI Companion guide](https://github.com/SomExE/minecraft-ai-companion) connects a separate player to a Java LAN world and exposes observations, inventory and actions through a local MCP bridge. Its [continuous-play guide](https://github.com/SomExE/minecraft-ai-companion/blob/main/docs/continuous-play.md) reports a Codex session controlling those tools. These are author-reported project results; we did not run them. This architecture explains how a model can act without manually pressing the desktop client's keys.

[Minecraft Codex Companion](https://github.com/Hakurei-git/minecraft-codex-companion) is another approach: a game mod provides an actor and executes validated tasks, connected to a local control service. These interfaces are Minecraft-specific and do not supply GRID support.

## Other game examples and timing

The [Portal agent explanation](https://github.com/cozyblaze/portal-agent/blob/main/docs/how-it-works.md) describes Codex controlling a modified SourcePauseTool through a local MCP server. It advances a chosen number of ticks, pauses while the model thinks, and returns screenshots plus position/view information. Its [Windows setup guide](https://github.com/cozyblaze/portal-agent/blob/main/docs/setup.md) is tied to particular Portal builds and plugins; it cannot be installed into GRID as a generic controller. The documented run also modifies movement settings, so it is an architecture example rather than a faithful original-physics measurement recipe.

[DOOM MCP](https://github.com/gunnargrosch/doom-mcp) embeds a source-available engine and exposes held actions for a chosen number of simulation ticks with returned state/images. GRID needs its own implementation if we want equivalent frame/tick control.

[Official OpenAI MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) supports local command-based servers. MCP transports tool calls/results; it does not itself provide input timing or game access. [OpenAI computer-use documentation](https://developers.openai.com/api/docs/guides/tools-computer-use) describes the observe/action/updated-screenshot loop. A third-party [Windows MCP controller](https://github.com/ChrisPulman/CodexComputerRunMCPServer) also documents a key-hold duration parameter; its GRID compatibility remains unverified and it was not installed.

## Recommended sequence for this project

1. Test one short, observed menu macro with explicit down/up timing. Use AutoHotkey v2 as the standard-tool option, or first validate the already compiled Rust backend. A bound trigger and the actual game response must each be verified.
2. Execute short, logged driving batches locally. Pause between batches where possible and return a fresh screenshot. Keep both single and simultaneous inputs available, with requested/observed durations and cancellation/release handling.
3. For handling measurements, recover GRID's input sampling/update timing and relevant telemetry. A future GRID-specific bridge can expose `observe`, `run_macro`, `pause` and eventually `advance_ticks`. These are proposed capabilities, not available tools. Standard keyboard macros cannot establish deterministic physics sampling by themselves.

No further manual driving demonstration is needed from the owner. The earlier Lacetti/Donington driving and profile persistence are confirmed; precise automated control and a fresh race-load trace remain pending.
