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
set "GRID_PLAY_LOG=local\playtest\manual-%RANDOM%-%RANDOM%.log"
echo Lacetti at Donington - experimental Rust/Bevy driving prototype
echo Up/W: throttle   Down/S: brake   Left/A and Right/D: steer
echo Space: handbrake   P: pause/resume   R: reset   Escape: exit
echo Click the game window once if keyboard input does not respond.
echo Drive slowly at first. Body/barrier collisions and lap timing are unfinished.
echo Private diagnostic log: %GRID_PLAY_LOG%
echo Loading the source track can take a little while...
if "%~2"=="" (
  "target\release\grid-app.exe" --drive --game-root "%GRID_PLAY_ROOT%" > "%GRID_PLAY_LOG%" 2>&1
) else (
  echo Experimental body contacts enabled. Heavy impacts can pause; R resets.
  if "%~3"=="" (
    "target\release\grid-app.exe" --drive --game-root "%GRID_PLAY_ROOT%" --body-contacts "%~2" > "%GRID_PLAY_LOG%" 2>&1
  ) else (
    echo Registered body impact events enabled for static barriers.
    if "%~4"=="" (
      "target\release\grid-app.exe" --drive --game-root "%GRID_PLAY_ROOT%" --body-contacts "%~2" --body-events "%~3" > "%GRID_PLAY_LOG%" 2>&1
    ) else (
      echo Experimental impact damage enabled.
      "target\release\grid-app.exe" --drive --game-root "%GRID_PLAY_ROOT%" --body-contacts "%~2" --body-events "%~3" --deformation "%~4" > "%GRID_PLAY_LOG%" 2>&1
    )
  )
)
set "GRID_PLAY_EXIT=%ERRORLEVEL%"
echo Prototype exited with code %GRID_PLAY_EXIT%.
echo Log saved to %GRID_PLAY_LOG%.
popd
pause
exit /b %GRID_PLAY_EXIT%
