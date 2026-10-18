@echo off
setlocal
rem Three-lap Lacetti race at Spa-Francorchamps against 11 AI opponents
rem (settings\race-spa.ini, docs\SPA.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" race-spa
exit /b %ERRORLEVEL%
