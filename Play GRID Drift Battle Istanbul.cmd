@echo off
setlocal
rem Drift Battle on the Istanbul Park intermediate layout (istanbul_int, route_1): two laps against 7 AI cars,
rem scored by drift points (settings\drift-battle-istanbul.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-battle-istanbul
exit /b %ERRORLEVEL%
