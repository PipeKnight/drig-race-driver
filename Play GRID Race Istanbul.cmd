@echo off
setlocal
rem Three-lap Lacetti race at Istanbul Park against 11 AI opponents
rem (settings\race-istanbul.ini, docs\ISTANBUL_PARK.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" race-istanbul
exit /b %ERRORLEVEL%
