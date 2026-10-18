@echo off
setlocal
rem Freestyle drift: 90 s alone in the Yokohama arena (route 3), scored by drift
rem points (settings\drift-freestyle.ini, docs\DISCIPLINES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" drift-freestyle
exit /b %ERRORLEVEL%
