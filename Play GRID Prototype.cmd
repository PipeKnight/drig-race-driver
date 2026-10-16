@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
set "GRID_PLAY_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Grid"
if not "%~1"=="" set "GRID_PLAY_ROOT=%~1"
if not exist "target\release\grid-app.exe" (
  echo The prototype has not been built in this checkout.
  echo Build it with: cargo build --release -p grid-app
  popd
  pause
  exit /b 2
)
if not exist "%GRID_PLAY_ROOT%\cars\lac\lac.ned" (
  echo GRID resources were not found at "%GRID_PLAY_ROOT%".
  echo Drag a shortcut with the game folder as an argument, or run this launcher with the folder path.
  popd
  pause
  exit /b 3
)
if not exist "local\playtest" mkdir "local\playtest"
if not exist "local\playtest" (
  echo Could not create the private playtest log folder.
  popd
  pause
  exit /b 4
)
set "GRID_TRIAL_ARGS="
if "%~5"=="3" set "GRID_TRIAL_ARGS=--time-trial-laps 3"
if not "%~5"=="" if not "%~5"=="3" (
  echo This launcher supports a three-lap time trial.
  popd
  exit /b 5
)
set "GRID_OPPONENT_ARGS="
if defined GRID_PLAY_OPPONENTS set "GRID_OPPONENT_ARGS=--opponents %GRID_PLAY_OPPONENTS%"
if defined GRID_PLAY_GRID set "GRID_OPPONENT_ARGS=%GRID_OPPONENT_ARGS% --grid-position %GRID_PLAY_GRID%"
if defined GRID_PLAY_OPPONENTS if exist "local\ai-driver-characteristics.tsv" set "GRID_OPPONENT_ARGS=%GRID_OPPONENT_ARGS% --ai-characteristics local\ai-driver-characteristics.tsv"
rem Driver aids (original options): GRID_PLAY_ASSISTS, e.g. tcs=off,stability=off,braking=on,gearbox=manual,
rem or a local\assists.txt settings file. Default: every aid on, automatic gearbox, keyboard steering.
set "GRID_ASSIST_ARGS="
if exist "local\assists.txt" set "GRID_ASSIST_ARGS=--assists-file local\assists.txt"
if defined GRID_PLAY_ASSISTS set "GRID_ASSIST_ARGS=--assists "%GRID_PLAY_ASSISTS%""
rem Controller settings: GRID_PLAY_INPUT, e.g. device=pad,steering_deadzone=0 (replaces an [input]
rem section of localssists.txt). A connected gamepad uses the game's own XInput pad preset.
set "GRID_INPUT_ARGS="
if defined GRID_PLAY_INPUT set "GRID_INPUT_ARGS=--input "%GRID_PLAY_INPUT%""
set "GRID_PLAY_LOG=local\playtest\manual-%RANDOM%-%RANDOM%.log"
echo Lacetti at Donington - experimental Rust/Bevy driving prototype
echo Up/W: throttle   Down/S: brake   Left/A and Right/D: steer
echo Space: handbrake   C: look back   P: pause/resume   R: reset   Escape: exit
echo Gamepad: left stick or D-pad steer   RT throttle   LT brake   B handbrake   Y look back   Start pause
if defined GRID_ASSIST_ARGS echo Driver aids: %GRID_ASSIST_ARGS%   Manual gearbox: X gear up, Z gear down (pad: A up, X down)
if defined GRID_PLAY_OPPONENTS echo %GRID_PLAY_OPPONENTS% opponent cars share the track, driven by the ported GRID AI (Normal difficulty).
echo Click the game window once if keyboard input does not respond.
if "%GRID_TRIAL_ARGS%"=="" (
  echo Practice timing starts at the start line. R resets the car and clears lap times.
) else (
  if defined GRID_PLAY_OPPONENTS (
    echo Three-lap race: wait for GO, race the AI cars for three laps, then view the result.
    echo R restarts the race.
  ) else (
    echo Three-lap time trial: wait for GO, complete three valid laps, then view results.
    echo R restarts the event; personal bests stay saved locally.
  )
)
echo Private diagnostic log: %GRID_PLAY_LOG%
echo Loading the source track can take a little while...
if "%~2"=="" (
  "target\release\grid-app.exe" --drive %GRID_TRIAL_ARGS% %GRID_ASSIST_ARGS% %GRID_INPUT_ARGS% --game-root "%GRID_PLAY_ROOT%" > "%GRID_PLAY_LOG%" 2>&1
) else (
  echo Experimental body contacts enabled. Terminal wheel damage disables driving; R resets.
  if "%~3"=="" (
    "target\release\grid-app.exe" --drive %GRID_TRIAL_ARGS% %GRID_ASSIST_ARGS% %GRID_INPUT_ARGS% --game-root "%GRID_PLAY_ROOT%" --body-contacts "%~2" %GRID_OPPONENT_ARGS% > "%GRID_PLAY_LOG%" 2>&1
  ) else (
    echo Registered body impact events enabled for static barriers.
    if "%~4"=="" (
      "target\release\grid-app.exe" --drive %GRID_TRIAL_ARGS% %GRID_ASSIST_ARGS% %GRID_INPUT_ARGS% --game-root "%GRID_PLAY_ROOT%" --body-contacts "%~2" --body-events "%~3" %GRID_OPPONENT_ARGS% > "%GRID_PLAY_LOG%" 2>&1
    ) else (
      echo Experimental impact damage enabled.
      "target\release\grid-app.exe" --drive %GRID_TRIAL_ARGS% %GRID_ASSIST_ARGS% %GRID_INPUT_ARGS% --game-root "%GRID_PLAY_ROOT%" --body-contacts "%~2" --body-events "%~3" --deformation "%~4" %GRID_OPPONENT_ARGS% > "%GRID_PLAY_LOG%" 2>&1
    )
  )
)
set "GRID_PLAY_EXIT=%ERRORLEVEL%"
echo Prototype exited with code %GRID_PLAY_EXIT%.
echo Log saved to %GRID_PLAY_LOG%.
popd
pause
exit /b %GRID_PLAY_EXIT%
