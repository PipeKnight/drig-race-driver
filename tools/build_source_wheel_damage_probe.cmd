@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 /DGRID_WHEEL_DAMAGE_SOURCE tools\probe_wheel_damage.c /Fo:local\behavior\source-wheel-damage-probe.obj /Fe:local\behavior\source-wheel-damage-probe.exe /link /BASE:0x10000000 /DYNAMICBASE advapi32.lib
exit /b %errorlevel%
