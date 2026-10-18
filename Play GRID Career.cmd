@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
rem Usage: Play GRID Career.cmd [GAME_FOLDER]
rem Opens the career screen (docs\CAREER_SCREEN.md): money, reputation, licences and
rem season, and the open career events the port can run. An event runs its races one
rem after another; after the last race the prize money and reputation are paid and
rem saved in local\career.save. Leaving a race (results Continue or Exit) returns to
rem the career screen.
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
set "GRID_PLAY_LOG=local\playtest\career-%RANDOM%-%RANDOM%.log"
echo GRID - experimental Rust/Bevy driving prototype: career
echo Career: Up/Down choose an event or race, Enter starts it, Escape closes the screen.
echo Gamepad: D-pad or left stick, A select, B back.
echo In the race: Up/W throttle, Down/S brake, Left/A and Right/D steer, Space handbrake,
echo Escape or P pause menu. Results Continue or Exit returns to the career screen.
echo Private diagnostic log: %GRID_PLAY_LOG%
"target\release\grid-app.exe" --career %GRID_ROOT_ARGS% > "%GRID_PLAY_LOG%" 2>&1
set "GRID_PLAY_EXIT=%ERRORLEVEL%"
echo Prototype exited with code %GRID_PLAY_EXIT%.
echo Log saved to %GRID_PLAY_LOG%.
popd
pause
exit /b %GRID_PLAY_EXIT%
