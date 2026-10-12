@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
if "%~1"=="regression-only" goto regression
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_triangle_manifold.c /Fo:local\behavior\triangle-manifold-probe.obj /Fe:local\behavior\triangle-manifold-probe.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
:regression
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_triangle_manifold.c /Fo:local\behavior\triangle-manifold-source-regression.obj /Fe:local\behavior\triangle-manifold-source-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_triangle_mesh_outer_sequence.c /Fo:local\behavior\triangle-manifold-outer-sequence-regression.obj /Fe:local\behavior\triangle-manifold-outer-sequence-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_triangle_mesh_sequence.c /Fo:local\behavior\triangle-manifold-distance-sequence-regression.obj /Fe:local\behavior\triangle-manifold-distance-sequence-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_triangle_mesh_query.c /Fo:local\behavior\triangle-manifold-outer-regression.obj /Fe:local\behavior\triangle-manifold-outer-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_triangle_mesh_distance.c /Fo:local\behavior\triangle-manifold-distance-regression.obj /Fe:local\behavior\triangle-manifold-distance-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_packed_mesh_contact.c /Fo:local\behavior\triangle-manifold-packed-regression.obj /Fe:local\behavior\triangle-manifold-packed-regression.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
exit /b %errorlevel%
