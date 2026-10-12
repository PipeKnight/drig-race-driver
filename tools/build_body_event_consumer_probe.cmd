@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_body_event_consumer.c /Fo:local\behavior\body-event-consumer-probe.obj /Fe:local\behavior\body-event-consumer-probe.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE advapi32.lib
exit /b %errorlevel%
