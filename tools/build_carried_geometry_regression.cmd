@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_physical_tick.c /Fo:local\behavior\carried-geometry-physical-tick-regression.obj /Fe:local\behavior\carried-geometry-physical-tick-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_world_physical_tick.c /Fo:local\behavior\carried-geometry-world-physical-tick-regression.obj /Fe:local\behavior\carried-geometry-world-physical-tick-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_grouped_physical_frame.c /Fo:local\behavior\carried-geometry-grouped-physical-frame-regression.obj /Fe:local\behavior\carried-geometry-grouped-physical-frame-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_grouped_physical_tick.c /Fo:local\behavior\carried-geometry-grouped-physical-tick-regression.obj /Fe:local\behavior\carried-geometry-grouped-physical-tick-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_shaped_physical_frame.c /Fo:local\behavior\carried-geometry-shaped-physical-frame-regression.obj /Fe:local\behavior\carried-geometry-shaped-physical-frame-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_shaped_physical_tick.c /Fo:local\behavior\carried-geometry-shaped-physical-tick-regression.obj /Fe:local\behavior\carried-geometry-shaped-physical-tick-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_whole_physical_world.c /Fo:local\behavior\carried-geometry-whole-physical-world-regression.obj /Fe:local\behavior\carried-geometry-whole-physical-world-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_carried_physical_world.c /Fo:local\behavior\carried-geometry-carried-physical-world-regression.obj /Fe:local\behavior\carried-geometry-carried-physical-world-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_reproduced_physical_world.c /Fo:local\behavior\carried-geometry-reproduced-physical-world-regression.obj /Fe:local\behavior\carried-geometry-reproduced-physical-world-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_geometric_physical_tick.c /Fo:local\behavior\carried-geometry-geometric-physical-tick-regression.obj /Fe:local\behavior\carried-geometry-geometric-physical-tick-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
exit /b 0
