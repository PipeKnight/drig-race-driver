@echo off
setlocal
rem Three-lap Lacetti race at Istanbul Park against 11 AI opponents
rem (settings\race-istanbul.ini, docs\ISTANBUL_PARK.md).
rem Needs the private CD01 file in local\; BP01 and DF01 are read from grid.exe.
if not exist "%~dp0local\body-contact-directions.bin" (
  echo Private body contact directions were not found in local\.
  echo See docs\DONINGTON_BODY_CONTACTS.md for preparation requirements.
  pause
  exit /b 6
)
call "%~dp0Play GRID Prototype.cmd" "%~1" race-istanbul
exit /b %ERRORLEVEL%
