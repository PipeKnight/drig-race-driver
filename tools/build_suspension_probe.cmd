@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_suspension.c /Fo:local\behavior\suspension-probe.obj /Fe:local\behavior\suspension-probe.exe /link /BASE:0x10000000 /DYNAMICBASE advapi32.lib
exit /b %errorlevel%
