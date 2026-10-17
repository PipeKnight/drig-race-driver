@echo off
setlocal
rem Three-lap Lacetti race at Long Beach against 11 AI opponents
rem (settings\race-long-beach.ini, docs\LONG_BEACH.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" race-long-beach
exit /b %ERRORLEVEL%
