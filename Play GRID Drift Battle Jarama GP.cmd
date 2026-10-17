@echo off
setlocal
rem Drift Battle on the Jarama GP layout (jarama_gp, route_0): two laps against 7 AI cars,
rem scored by drift points (settings\drift-battle-jarama-gp.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-battle-jarama-gp
exit /b %ERRORLEVEL%
