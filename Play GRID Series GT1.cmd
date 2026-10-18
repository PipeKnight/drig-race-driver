@echo off
setlocal
rem Three-lap GT1 race at the Nurburgring GP (DBR9, Corvettes, S7R, CCGT, Murcielago)
rem (settings\series-gt1.ini, docs\SERIES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" series-gt1
exit /b %ERRORLEVEL%
