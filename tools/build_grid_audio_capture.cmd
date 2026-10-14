@echo off
call "C:\BuildTools\GRID\VC\Auxiliary\Build\vcvarsamd64_x86.bat"
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /O2 /MT /std:c++20 /EHsc tools\capture_grid_audio.cpp /Fo:local\audio\capture-grid-audio.obj /Fe:local\audio\capture-grid-audio.exe /link /BASE:0x10000000 /DYNAMICBASE /MANIFESTUAC:"level='asInvoker' uiAccess='false'" ole32.lib mmdevapi.lib
exit /b %errorlevel%
