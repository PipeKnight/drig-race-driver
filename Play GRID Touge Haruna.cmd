@echo off
setlocal
rem Touge run down Haruna (route 1, mid to bottom) against one AI car
rem (settings\touge-haruna.ini, docs\TOUGE.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" touge-haruna
exit /b %ERRORLEVEL%
