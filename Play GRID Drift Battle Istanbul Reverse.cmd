@echo off
setlocal
rem Drift Battle on the Istanbul Park intermediate layout reversed (istanbul_int_r, route_3): two laps against 7 AI cars,
rem scored by drift points (settings\drift-battle-istanbul-reverse.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-battle-istanbul-reverse
exit /b %ERRORLEVEL%
