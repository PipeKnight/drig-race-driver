@echo off
setlocal
rem Drift Battle on the Nurburgring sprint layout (nurburgring_spr, route_1): two laps against 7 AI cars,
rem scored by drift points (settings\drift-battle-nurburgring.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-battle-nurburgring
exit /b %ERRORLEVEL%
