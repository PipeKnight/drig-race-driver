@echo off
setlocal
rem Three-lap Le Mans Series race at Donington GP: 15 cars of four classes
rem (settings\series-le-mans.ini, docs\SERIES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" series-le-mans
exit /b %ERRORLEVEL%
