@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b %errorlevel%
cl /nologo /W4 /WX /O2 /MT /std:c17 /Fo:local/behavior/triangle-kernel-probe.obj /Fe:local/behavior/triangle-kernel-probe.exe tools/probe_triangle_kernel.c /link /BASE:0x10000000 /DYNAMICBASE advapi32.lib
exit /b %errorlevel%
