@echo off
setlocal
rem Drift Battle on Yokohama Docks (yoko_v1, route_0): two laps against 7 AI cars,
rem scored by drift points (settings\drift-battle-yokohama.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-battle-yokohama
exit /b %ERRORLEVEL%
