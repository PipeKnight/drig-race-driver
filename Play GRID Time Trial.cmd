@echo off
setlocal
for %%F in (body-contact-directions.bin body-impact-parameters.bin deformation-parameters.bin) do (
  if not exist "%~dp0local\%%F" (
    echo The private time-trial input local\%%F is missing.
    echo See docs\TIME_TRIAL_EVENT.md for preparation requirements.
    pause
    exit /b 2
  )
)
call "%~dp0Play GRID Prototype.cmd" "%~1" "%~dp0local\body-contact-directions.bin" "%~dp0local\body-impact-parameters.bin" "%~dp0local\deformation-parameters.bin" "3"
exit /b %ERRORLEVEL%
