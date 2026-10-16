@echo off
setlocal
rem Three-lap Lacetti/Donington race against AI opponent cars in the shared physical world.
rem Needs the private CD01/BP01 files in local\; local\deformation-parameters.bin adds impact damage.
if not exist "%~dp0local\body-contact-directions.bin" (
  echo Private body contact directions were not found in local\.
  pause
  exit /b 6
)
if not exist "%~dp0local\body-impact-parameters.bin" (
  echo Private body impact parameters were not found in local\.
  pause
  exit /b 7
)
rem The original fields 12 touring cars (11 AI opponents and the player).
if not defined GRID_PLAY_OPPONENTS set "GRID_PLAY_OPPONENTS=11"
rem Start from the back of the grid unless GRID_PLAY_GRID chooses a position.
if not defined GRID_PLAY_GRID set /a GRID_PLAY_GRID=GRID_PLAY_OPPONENTS+1
if exist "%~dp0local\deformation-parameters.bin" (
  call "%~dp0Play GRID Prototype.cmd" "%~1" "%~dp0local\body-contact-directions.bin" "%~dp0local\body-impact-parameters.bin" "%~dp0local\deformation-parameters.bin" 3
) else (
  call "%~dp0Play GRID Prototype.cmd" "%~1" "%~dp0local\body-contact-directions.bin" "%~dp0local\body-impact-parameters.bin" "" 3
)
exit /b %ERRORLEVEL%
