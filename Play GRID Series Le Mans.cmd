@echo off
setlocal
rem Three-lap Le Mans Series race at Donington GP: 15 cars of four classes
rem (settings\series-le-mans.ini, docs\SERIES.md). The field includes Courage
rem LC70 prototypes, which still ground at speed, so it runs as experimental.
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
set "GRID_SERIES_EXPERIMENTAL=1"
call "%~dp0Play GRID Prototype.cmd" "%~1" series-le-mans
exit /b %ERRORLEVEL%
