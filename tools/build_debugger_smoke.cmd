@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 tools\debugger_smoke.c /Fo:local\behavior\debugger-smoke.obj /Fe:local\behavior\debugger-smoke.exe
exit /b %errorlevel%
