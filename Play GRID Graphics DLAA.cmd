@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
set "GRID_GRAPHICS_EXE=target\release\grid-app.exe"
set "GRID_GRAPHICS_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Grid"
if not "%~1"=="" set "GRID_GRAPHICS_ROOT=%~1"
set "GRID_GRAPHICS_RESOLUTION=1920x1080"
if not "%~2"=="" set "GRID_GRAPHICS_RESOLUTION=%~2"
set "GRID_GRAPHICS_AA=dlaa"
if /I "%~3"=="smaa" set "GRID_GRAPHICS_AA=smaa"
if /I "%~3"=="taa" set "GRID_GRAPHICS_AA=taa"
if not exist "%GRID_GRAPHICS_EXE%" (
  echo The current release is missing. Build grid-app with --features dlaa for native DLAA.
  echo See docs\NATIVE_DLAA.md.
  popd
  pause
  exit /b 2
)
if /I "%GRID_GRAPHICS_AA%"=="dlaa" if not exist "target\release\nvngx_dlss.dll" (
  echo The private DLAA runtime is missing beside the executable.
  popd
  pause
  exit /b 3
)
if not exist "local\graphics\" mkdir "local\graphics"
if not exist "local\graphics\" (
  echo Could not create the private graphics log folder.
  popd
  pause
  exit /b 4
)
set "GRID_GRAPHICS_GLASS=--glass-pilot off"
if /I "%GRID_GRAPHICS_AA%"=="dlaa" if /I "%GRID_GRAPHICS_RESOLUTION%"=="1920x1080" set "GRID_GRAPHICS_GLASS=--glass-pilot base-mip-zero"
rem This native integration uses Vulkan. DLL discovery needs no global SDK setup.
set "WGPU_BACKEND=vulkan"
set "DLSS_SDK="
set "GRID_GRAPHICS_CONTACTS=--body-contacts derive"
if exist "local\body-contact-directions.bin" set "GRID_GRAPHICS_CONTACTS=--body-contacts local\body-contact-directions.bin"
if exist "local\body-impact-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --body-events local\body-impact-parameters.bin"
if exist "local\deformation-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --deformation local\deformation-parameters.bin"
set "GRID_GRAPHICS_LOG=local\graphics\manual-dlaa-%RANDOM%-%RANDOM%.log"
rem A leftover DLL does not prove the release was compiled with native DLAA.
if /I "%GRID_GRAPHICS_AA%"=="dlaa" (
  "%GRID_GRAPHICS_EXE%" --aa dlaa --help > "%GRID_GRAPHICS_LOG%" 2>&1
  if errorlevel 1 (
    type "%GRID_GRAPHICS_LOG%"
    echo This executable could not accept DLAA. Rebuild with --features dlaa.
    popd
    pause
    exit /b 5
  )
)
echo Current enhanced graphics release; AA: %GRID_GRAPHICS_AA%; resolution: %GRID_GRAPHICS_RESOLUTION%.
echo No upscaling or frame generation. Unsupported devices use SMAA; see log.
echo Arrows/WASD drive; Space handbrake; Escape/P pause; R restart.
echo Private log: %GRID_GRAPHICS_LOG%
"%GRID_GRAPHICS_EXE%" --drive --route tracks/europe/donington/route_0 --car lac --graphics enhanced --ibl source --sun source --trees coverage --numbers source --body-occlusion source-ambient --body-specular source-base %GRID_GRAPHICS_GLASS% --aa "%GRID_GRAPHICS_AA%" --resolution "%GRID_GRAPHICS_RESOLUTION%" --game-root "%GRID_GRAPHICS_ROOT%" %GRID_GRAPHICS_CONTACTS% > "%GRID_GRAPHICS_LOG%" 2>&1
set "GRID_GRAPHICS_EXIT=%ERRORLEVEL%"
echo Exited with code %GRID_GRAPHICS_EXIT%.
popd
pause
exit /b %GRID_GRAPHICS_EXIT%
