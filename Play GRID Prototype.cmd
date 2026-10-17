@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
rem Usage: Play GRID Prototype.cmd [GAME_FOLDER] [PROFILE]
rem PROFILE names a settings file in settings\ (default: practice). The app reads
rem local\settings.ini first, then the profile (docs\SETTINGS.md). Without GAME_FOLDER the
rem app uses [game] root from local\settings.ini, or searches the Steam libraries.
set "GRID_PLAY_PROFILE=practice"
if not "%~2"=="" set "GRID_PLAY_PROFILE=%~2"
if not exist "target\release\grid-app.exe" (
  echo The prototype has not been built in this checkout.
  echo Build it with: cargo build --release -p grid-app
  popd
  pause
  exit /b 2
)
if not exist "settings\%GRID_PLAY_PROFILE%.ini" (
  echo Unknown settings profile "%GRID_PLAY_PROFILE%": settings\%GRID_PLAY_PROFILE%.ini is missing.
  popd
  pause
  exit /b 5
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
rem Older overrides, still honoured as command-line flags: local\assists.txt, and the
rem variables GRID_PLAY_ASSISTS (e.g. tcs=off,gearbox=manual), GRID_PLAY_INPUT (e.g. device=pad),
rem GRID_PLAY_OPPONENTS and GRID_PLAY_GRID. Prefer [assists], [input] and [drive] in local\settings.ini.
set "GRID_EXTRA_ARGS="
if exist "local\assists.txt" set "GRID_EXTRA_ARGS=--assists-file local\assists.txt"
if defined GRID_PLAY_ASSISTS set "GRID_EXTRA_ARGS=--assists "%GRID_PLAY_ASSISTS%""
if defined GRID_PLAY_INPUT set "GRID_EXTRA_ARGS=%GRID_EXTRA_ARGS% --input "%GRID_PLAY_INPUT%""
if defined GRID_PLAY_OPPONENTS set "GRID_EXTRA_ARGS=%GRID_EXTRA_ARGS% --opponents %GRID_PLAY_OPPONENTS%"
if defined GRID_PLAY_GRID set "GRID_EXTRA_ARGS=%GRID_EXTRA_ARGS% --grid-position %GRID_PLAY_GRID%"
set "GRID_PLAY_LOG=local\playtest\manual-%RANDOM%-%RANDOM%.log"
echo GRID - experimental Rust/Bevy driving prototype (car and circuit from the settings profile; shown at the top left in the game)
echo Up/W: throttle   Down/S: brake   Left/A and Right/D: steer
echo Space: handbrake   C: look back   Escape or P: pause menu   R: restart
echo Gamepad: left stick or D-pad steer   RT throttle   LT brake   B handbrake   Y look back   Start pause menu
echo Manual gearbox (gearbox=manual in [assists]): X gear up, Z gear down (pad: A up, X down)
echo Settings profile: settings\%GRID_PLAY_PROFILE%.ini %GRID_EXTRA_ARGS%
echo Click the game window once if keyboard input does not respond.
if "%GRID_PLAY_PROFILE%"=="practice" echo Practice timing starts at the start line. R resets the car and clears lap times.
if "%GRID_PLAY_PROFILE%"=="body-contacts" echo Experimental body contacts and impact damage. Terminal wheel damage disables driving; R resets.
if "%GRID_PLAY_PROFILE%"=="time-trial" (
  echo Three-lap time trial: wait for GO, complete three valid laps, then view results.
  echo R restarts the event; personal bests stay saved locally.
)
if "%GRID_PLAY_PROFILE%"=="race" (
  echo Three-lap race: wait for GO, race the AI cars for three laps, then view the result.
  echo The opponents are driven by the ported GRID AI ^(Normal difficulty^). R restarts the race.
)
if "%GRID_PLAY_PROFILE:~0,12%"=="drift-battle" (
  echo Drift Battle: two laps against 7 AI cars. Drift to score: hold an angle above 15 degrees at
  echo 50 mph or more; long drifts, combos and close drifts past the flags score more; a collision
  echo loses the combo. The race leader scores x3, the last car x1. Results are ordered by score.
)
echo Private diagnostic log: %GRID_PLAY_LOG%
echo Loading the source track can take a little while...
"target\release\grid-app.exe" --settings "settings\%GRID_PLAY_PROFILE%.ini" %GRID_ROOT_ARGS% %GRID_EXTRA_ARGS% > "%GRID_PLAY_LOG%" 2>&1
set "GRID_PLAY_EXIT=%ERRORLEVEL%"
echo Prototype exited with code %GRID_PLAY_EXIT%.
echo Log saved to %GRID_PLAY_LOG%.
popd
pause
exit /b %GRID_PLAY_EXIT%
