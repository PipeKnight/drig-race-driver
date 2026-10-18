@echo off
setlocal
rem Three-lap Lacetti race at Yokohama Docks against 11 AI opponents
rem (settings\race-yokohama.ini, docs\YOKOHAMA.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" race-yokohama
exit /b %ERRORLEVEL%
