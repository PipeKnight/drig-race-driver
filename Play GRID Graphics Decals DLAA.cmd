@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
set "GRID_GRAPHICS_EXE=local\graphics\decal-alpha01\grid-decal.exe"
set "GRID_GRAPHICS_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Grid"
if not "%~1"=="" set "GRID_GRAPHICS_ROOT=%~1"
set "GRID_GRAPHICS_RESOLUTION=1920x1080"
if not "%~2"=="" set "GRID_GRAPHICS_RESOLUTION=%~2"
set "GRID_GRAPHICS_AA=dlaa"
if /I "%~3"=="smaa" set "GRID_GRAPHICS_AA=smaa"
if /I "%~3"=="taa" set "GRID_GRAPHICS_AA=taa"
if not exist "%GRID_GRAPHICS_EXE%" (
  echo Optional native graphics build is missing. See docs\GRAPHICS_DEFECTS.md.
  popd
  pause
  exit /b 2
)
if not exist "local\graphics\decal-alpha01\nvngx_dlss.dll" (
  echo The private DLAA runtime is missing beside the executable.
  popd
  pause
  exit /b 3
)
rem This native integration uses Vulkan. DLL discovery needs no global SDK setup.
set "WGPU_BACKEND=vulkan"
set "DLSS_SDK="
set "GRID_DLSS_PRESET_HINT="
set "GRID_DLSS_EXPOSURE="
set "GRID_DLSS_FRAME_TIME_MS="
set "GRID_GRAPHICS_CONTACTS=--body-contacts derive"
if exist "local\body-contact-directions.bin" set "GRID_GRAPHICS_CONTACTS=--body-contacts local\body-contact-directions.bin"
if exist "local\body-impact-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --body-events local\body-impact-parameters.bin"
if exist "local\deformation-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --deformation local\deformation-parameters.bin"
set "GRID_GRAPHICS_LOG=local\graphics\manual-decals-dlaa-%RANDOM%-%RANDOM%.log"
echo Source decal transparency fix and brake/reverse lamp signals; AA: %GRID_GRAPHICS_AA%; resolution: %GRID_GRAPHICS_RESOLUTION%.
echo No upscaling or frame generation. Unsupported devices use SMAA; see log.
echo Arrows/WASD drive; Space handbrake; Escape/P pause; R restart.
echo Private log: %GRID_GRAPHICS_LOG%
"%GRID_GRAPHICS_EXE%" --drive --route tracks/europe/donington/route_0 --car lac --graphics enhanced --ibl source --sun source --trees coverage --numbers source --body-occlusion source-ambient --body-specular source-base --reflections probe --probe-update moving --probe-size 1024 --paint-environment 0.5 --lamp-reflectors combined --lamp-signals live --aa "%GRID_GRAPHICS_AA%" --resolution "%GRID_GRAPHICS_RESOLUTION%" --game-root "%GRID_GRAPHICS_ROOT%" %GRID_GRAPHICS_CONTACTS% > "%GRID_GRAPHICS_LOG%" 2>&1
set "GRID_GRAPHICS_EXIT=%ERRORLEVEL%"
echo Exited with code %GRID_GRAPHICS_EXIT%.
popd
pause
exit /b %GRID_GRAPHICS_EXIT%
