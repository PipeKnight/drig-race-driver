@echo off
setlocal
rem Drift Battle on the Long Beach drift circuit reversed (longbeach_cl_r, route_4): two laps against 7 AI cars,
rem scored by drift points (settings\drift-battle-long-beach-reverse.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-battle-long-beach-reverse
exit /b %ERRORLEVEL%
