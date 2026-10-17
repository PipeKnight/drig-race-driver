@echo off
setlocal
rem Drift Battle: two laps of the Long Beach drift circuit against 7 AI cars,
rem scored by drift points (settings\drift-battle.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-battle
exit /b %ERRORLEVEL%
