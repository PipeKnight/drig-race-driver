@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
rem Usage: Play GRID.cmd [GAME_FOLDER]
rem Opens the in-game event menu (Race Day, docs\EVENT_MENU.md): pick the discipline,
rem car, location, route and laps, then Race Options and Go To Race. Leaving a race
rem (results Continue or Exit) returns to the menu. The other Play GRID launchers still
rem start their event directly. The app reads local\settings.ini (the menu saves its
rem last choice there, in [event]).
if not exist "target\release\grid-app.exe" (
  echo The prototype has not been built in this checkout.
  echo Build it with: cargo build --release -p grid-app
  popd
  pause
  exit /b 2
)
set "GRID_ROOT_ARGS="
if not "%~1"=="" (
  if not exist "%~1\cars\lac\lac.ned" (
    echo GRID resources were not found at "%~1".
    echo Drag a shortcut with the game folder as an argument, or run this launcher with the folder path.
    popd
    pause
    exit /b 3
  )
  set "GRID_ROOT_ARGS=--game-root "%~1""
)
if not exist "local\playtest" mkdir "local\playtest"
if not exist "local\playtest" (
  echo Could not create the private playtest log folder.
  popd
  pause
  exit /b 4
)
set "GRID_PLAY_LOG=local\playtest\menu-%RANDOM%-%RANDOM%.log"
echo GRID - experimental Rust/Bevy driving prototype: event menu
echo Menu: Up/Down choose a row, Left/Right change it, Enter on Continue starts the event,
echo Escape closes the menu. Mouse: click a row, click it again to change it.
echo Gamepad: D-pad or left stick, A select, B back.
echo In the race: Up/W throttle, Down/S brake, Left/A and Right/D steer, Space handbrake,
echo Escape or P pause menu. Results Continue or Exit returns to the menu.
echo Private diagnostic log: %GRID_PLAY_LOG%
"target\release\grid-app.exe" --menu %GRID_ROOT_ARGS% > "%GRID_PLAY_LOG%" 2>&1
set "GRID_PLAY_EXIT=%ERRORLEVEL%"
echo Prototype exited with code %GRID_PLAY_EXIT%.
echo Log saved to %GRID_PLAY_LOG%.
popd
pause
exit /b %GRID_PLAY_EXIT%
