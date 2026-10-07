@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b %errorlevel%
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 /Fo:local/behavior/ground-placement-probe.obj /Fe:local/behavior/ground-placement-probe.exe tools/probe_ground_placement.c /link /BASE:0x10000000 /DYNAMICBASE advapi32.lib
exit /b %errorlevel%
