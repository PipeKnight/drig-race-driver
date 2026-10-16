@echo off
setlocal
rem Three-lap time trial with body contacts and damage (settings\time-trial.ini).
rem Needs the private CD01 file in local\; BP01 and DF01 are read from grid.exe.
if not exist "%~dp0local\body-contact-directions.bin" (
  echo The private time-trial input local\body-contact-directions.bin is missing.
  echo See docs\TIME_TRIAL_EVENT.md for preparation requirements.
  pause
  exit /b 2
)
call "%~dp0Play GRID Prototype.cmd" "%~1" time-trial
exit /b %ERRORLEVEL%
