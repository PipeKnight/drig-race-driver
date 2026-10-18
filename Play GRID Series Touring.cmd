@echo off
setlocal
rem Three-lap Touring race at Istanbul Park reverse (BMW 320si and Lacetti)
rem (settings\series-touring.ini, docs\SERIES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" series-touring
exit /b %ERRORLEVEL%
