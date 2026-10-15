@echo off
setlocal
rem Lacetti/Donington practice with opponent cars in the shared physical world.
rem Needs the private CD01/BP01 files in local\; deformation is not supported with opponents yet.
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
if not defined GRID_PLAY_OPPONENTS set "GRID_PLAY_OPPONENTS=3"
call "%~dp0Play GRID Prototype.cmd" "%~1" "%~dp0local\body-contact-directions.bin" "%~dp0local\body-impact-parameters.bin"
exit /b %ERRORLEVEL%
