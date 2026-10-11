@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_standard_properties.c /Fo:local\behavior\standard-properties-probe.obj /Fe:local\behavior\standard-properties-probe.exe /link /BASE:0x10000000 /DYNAMICBASE /MANIFESTUAC:level='asInvoker' advapi32.lib
exit /b %errorlevel%
