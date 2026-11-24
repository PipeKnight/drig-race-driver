# DRIG for Windows: player notes

Source of the Windows section of the release notes. Nothing packages this
file: the release publishes the program itself, `DRIG-vVERSION-windows-x86_64.exe`,
with `DRIG-vVERSION-LICENSES.txt` and `SHA256SUMS.txt` (tools/package_desktop.ps1,
docs/SETTINGS.md section 6).

DRIG is an independent, open-source engine for Race Driver: GRID (2008),
written in Rust. It is not made, endorsed or supported by Codemasters.

No game data is included. DRIG reads the files of your own purchased copy of
GRID (the Windows version, for example from Steam) where it is installed.

## Start

1. Install GRID (2008) on this computer.
2. Put the DRIG exe in a folder you can write to, for example a new folder on
   your desktop. DRIG-data is created next to it on the first start.
3. Start the exe. The menu opens.

You can check the download against SHA256SUMS.txt, for example with
`Get-FileHash DRIG-vVERSION-windows-x86_64.exe` in PowerShell.

DRIG looks for GRID in your Steam libraries. If it is installed elsewhere,
pass the game folder each time (a shortcut or a launcher can do this):

    DRIG-vVERSION-windows-x86_64.exe --game-root "D:\Games\GRID"

or write it once into DRIG-data\settings.ini:

    [game]
    root = D:\Games\GRID

Career mode starts with the `--career` flag; the menu has no career entry
yet.

`--help` lists every player option: full screen or window, resolution, render
scale, quality, starting a race on a track directly.

## Your data

Settings, records, replays and the log (drig.log) are kept in the DRIG-data
folder next to the exe, so the whole game stays in one folder you can move or
delete. If that folder cannot be written (for example under Program Files),
DRIG uses %APPDATA%\GRID Reassembly instead. `--data-dir DIR` picks another
folder.

Earlier test builds kept their data in %APPDATA%\GRID Reassembly. To keep
it, copy settings.ini and the playtest folder from there into DRIG-data.

## Updating

In-game updates are not available yet. Download the new exe into the same
folder and delete the old one: both versions use the same DRIG-data.

## Licence

DRIG is free software under the GNU General Public License, version 3 or
later. DRIG-vVERSION-LICENSES.txt holds that licence and the notices of the
libraries and the font the exe includes.

Source code: https://github.com/PipeKnight/drig-race-driver
