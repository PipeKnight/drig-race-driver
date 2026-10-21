@echo off
setlocal
rem Three-lap Prestige race at Istanbul Park GP (route_0, istanbul_gp)
rem (settings\series-prestige.ini, docs\SERIES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" series-prestige
exit /b %ERRORLEVEL%
