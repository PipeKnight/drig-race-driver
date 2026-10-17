@echo off
setlocal
rem Free practice with body contacts and damage (settings\body-contacts.ini).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" body-contacts
exit /b %ERRORLEVEL%
