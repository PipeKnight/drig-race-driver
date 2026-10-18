@echo off
setlocal
rem Three-lap Pro Tuned race at Donington National (Supra, Skyline, NSX-R, 350Z)
rem (settings\series-pro-tuned.ini, docs\SERIES.md).
rem CD01, BP01 and DF01 are read from grid.exe (body_contacts = derive).
call "%~dp0Play GRID Prototype.cmd" "%~1" series-pro-tuned
exit /b %ERRORLEVEL%
