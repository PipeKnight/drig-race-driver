@echo off
setlocal
rem Demolition Derby: six laps of the Detroit stadium against 11 AI stock cars,
rem disqualification for a flipped or stopped car (settings\demolition-derby.ini, docs\DEMOLITION_DERBY.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" demolition-derby
exit /b %ERRORLEVEL%
