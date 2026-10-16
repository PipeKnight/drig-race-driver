@echo off
setlocal
rem Free practice with body contacts and damage (settings\body-contacts.ini).
rem Needs the private CD01 file in local\; BP01 and DF01 are read from grid.exe.
if not exist "%~dp0local\body-contact-directions.bin" (
  echo Locally prepared private body-contact sampling data is missing.
  echo See docs\DONINGTON_BODY_CONTACTS.md for preparation requirements.
  pause
  exit /b 2
)
call "%~dp0Play GRID Prototype.cmd" "%~1" body-contacts
exit /b %ERRORLEVEL%
