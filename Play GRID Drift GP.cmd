@echo off
setlocal
rem Drift GP: two laps of the Long Beach drift circuit alone on the track against
rem a virtual rival, scored by drift points (settings\drift-gp.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-gp
exit /b %ERRORLEVEL%
