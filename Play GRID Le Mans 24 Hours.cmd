@echo off
setlocal
rem Le Mans 24 Hours: 12 minutes of real time running 24 hours of day on
rem Circuit de la Sarthe, DBR9 in the 16-car four-class field, class results
rem (settings\le-mans-24.ini, docs\LE_MANS.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" le-mans-24
exit /b %ERRORLEVEL%
