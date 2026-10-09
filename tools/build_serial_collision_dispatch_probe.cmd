@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 tools\probe_serial_collision_dispatch.c /Fo:local\behavior\serial-collision-dispatch-probe.obj /Fe:local\behavior\serial-collision-dispatch-probe.exe /link /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:"level='asInvoker' uiAccess='false'" advapi32.lib
exit /b %errorlevel%
