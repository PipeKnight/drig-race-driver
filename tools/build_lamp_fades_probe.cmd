@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
if not exist local\graphics\lamp-state01 mkdir local\graphics\lamp-state01
cl /nologo /W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2 tools\probe_lamp_fades.c /Fo:local\graphics\lamp-state01\lamp-fades-validated.obj /Fe:local\graphics\lamp-state01\lamp-fades-validated.exe /link /STACK:16777216 /BASE:0x10000000 /DYNAMICBASE /MANIFEST:EMBED /MANIFESTUAC:level='asInvoker' advapi32.lib
exit /b %errorlevel%
