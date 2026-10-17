@echo off
setlocal
rem Three-lap Lacetti/Donington race against 11 AI opponents (settings\race.ini).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" race
exit /b %ERRORLEVEL%
