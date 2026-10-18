@echo off
setlocal
rem Two-lap Lacetti race on Circuit de la Sarthe against 11 AI opponents
rem (settings\race-le-mans.ini, docs\LE_MANS.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" race-le-mans
exit /b %ERRORLEVEL%
