@echo off
setlocal
if not exist "%~dp0local\body-contact-directions.bin" (
  echo Locally prepared private body-contact sampling data is missing.
  echo See docs\DONINGTON_BODY_CONTACTS.md for preparation requirements.
  pause
  exit /b 2
)
call "%~dp0Play GRID Prototype.cmd" "%~1" "%~dp0local\body-contact-directions.bin"
exit /b %ERRORLEVEL%
