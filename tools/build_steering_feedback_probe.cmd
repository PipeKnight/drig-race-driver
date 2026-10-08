@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_steering_feedback.c /Fo:local\behavior\steering-feedback-probe.obj /Fe:local\behavior\steering-feedback-probe.exe /link /BASE:0x10000000 /DYNAMICBASE advapi32.lib
exit /b %errorlevel%
