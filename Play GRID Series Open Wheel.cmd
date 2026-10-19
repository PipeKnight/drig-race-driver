@echo off
setlocal
rem Three-lap Open Wheel race at Donington Park GP (route_0, donington_gp)
rem (settings\series-open-wheel.ini, docs\SERIES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" series-open-wheel
exit /b %ERRORLEVEL%
