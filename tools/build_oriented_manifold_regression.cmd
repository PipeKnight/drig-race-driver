@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
for %%F in (triangle_manifold triangle_mesh_distance triangle_mesh_sequence triangle_mesh_query triangle_mesh_outer_sequence packed_mesh_contact packed_mesh_complete manifold_sequence) do (
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_%%F.c /Fo:local\behavior\oriented-regression-%%F.obj /Fe:local\behavior\oriented-regression-%%F.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
if errorlevel 1 exit /b 1
)
