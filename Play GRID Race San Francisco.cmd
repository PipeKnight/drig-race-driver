@echo off
setlocal
rem Three-lap Lacetti race at San Francisco against 11 AI opponents
rem (settings\race-san-francisco.ini, docs\STREET_CIRCUITS.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" race-san-francisco
exit /b %ERRORLEVEL%
