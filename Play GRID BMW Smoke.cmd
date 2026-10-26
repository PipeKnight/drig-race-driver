@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
set "GRID_GRAPHICS_EXE=local\graphics\playable-bmw-smoke01\grid-bmw-smoke.exe"
set "GRID_GRAPHICS_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Grid"
if not "%~1"=="" set "GRID_GRAPHICS_ROOT=%~1"
set "GRID_GRAPHICS_DATA=local\graphics\playable-bmw-smoke01\manual"
set "GRID_GRAPHICS_RESOLUTION=1920x1080"
if not "%~2"=="" set "GRID_GRAPHICS_RESOLUTION=%~2"
if not exist "%GRID_GRAPHICS_EXE%" (
  echo The private smoke comparison build is missing.
  popd
  pause
  exit /b 2
)
set "WGPU_BACKEND=vulkan"
set "DLSS_SDK="
set "GRID_DLSS_PRESET_HINT="
set "GRID_DLSS_EXPOSURE="
set "GRID_DLSS_FRAME_TIME_MS="
set "GRID_AUTOPILOT="
set "GRID_PROP_SCRIPT="
set "GRID_FREE_LOOK="
set "GRID_GRAPHICS_LOG=local\graphics\manual-bmw-smoke-%RANDOM%-%RANDOM%.log"
echo BMW / Istanbul live glass and lamp smoke-order trial. Baseline graphics; not DLAA.
echo Arrows/WASD drive; Space handbrake; Escape/P pause; R restart; C camera.
echo Body contacts/events/deformation must be disabled; incompatible saved settings are rejected.
echo Private data: %GRID_GRAPHICS_DATA%
echo Private log: %GRID_GRAPHICS_LOG%
"%GRID_GRAPHICS_EXE%" --drive --route tracks/europe/istanbul_park/route_0 --car bmw --graphics baseline --aa taa --reflections sky --input device=keyboard --data-dir "%GRID_GRAPHICS_DATA%" --car-transparent-sort off --smoke-order groups --skid-order off --smoke-ct off --smoke-boundary grouped-live --smoke-compositor off --ground-skid-pass off --resolution "%GRID_GRAPHICS_RESOLUTION%" --game-root "%GRID_GRAPHICS_ROOT%" > "%GRID_GRAPHICS_LOG%" 2>&1
set "GRID_GRAPHICS_EXIT=%ERRORLEVEL%"
echo Exited with code %GRID_GRAPHICS_EXIT%.
popd
pause
exit /b %GRID_GRAPHICS_EXIT%