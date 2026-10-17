@echo off
setlocal
rem Three-lap time trial with body contacts and damage (settings\time-trial.ini).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" time-trial
exit /b %ERRORLEVEL%
