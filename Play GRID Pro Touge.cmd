@echo off
setlocal
rem Pro Touge on Haruna: two runs against one AI car, downhill then uphill,
rem penalties added, results by total time (settings\pro-touge.ini, docs\TOUGE.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" pro-touge
exit /b %ERRORLEVEL%
