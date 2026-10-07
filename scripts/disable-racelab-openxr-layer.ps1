# Disables RaceLab OpenXR layer (DWORD 1 = disabled). Run as Administrator.
reg add "HKLM\Software\Khronos\OpenXR\1\ApiLayers\Implicit" /v "C:\Program Files\Racelab VR\XR_APILAYER_app_racelab_Overlay64.json" /t REG_DWORD /d 1 /f
if ($LASTEXITCODE -eq 0) { Write-Host "RaceLab OpenXR layer disabled." -ForegroundColor Green } else { Write-Host "Failed — need Admin?" -ForegroundColor Red }
pause
