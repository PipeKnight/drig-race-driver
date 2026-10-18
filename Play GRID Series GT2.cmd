@echo off
setlocal
rem Three-lap GT2 race at Istanbul Park intermediate (Spyker C8, Panoz Esperante)
rem (settings\series-gt2.ini, docs\SERIES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" series-gt2
exit /b %ERRORLEVEL%
