# CI smoke test: launch the packaged Windows app, verify the process stays
# alive and a main window is created, then terminate it.
# Pure ASCII on purpose (PS 5.1 misparses BOM-less UTF-8 with Chinese).
param(
  [Parameter(Mandatory = $true)][string]$AppExe,
  [int]$WindowTimeoutSec = 30,
  [int]$AliveSec = 10
)

$ErrorActionPreference = 'Stop'

$exe = (Resolve-Path $AppExe).Path
$workDir = Split-Path $exe -Parent
Write-Host "smoke: launching $exe"
$p = Start-Process -FilePath $exe -WorkingDirectory $workDir -PassThru

try {
  $deadline = (Get-Date).AddSeconds($WindowTimeoutSec)
  $handle = [IntPtr]::Zero
  while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $p.Refresh()
    if ($p.HasExited) { throw "app exited early with code $($p.ExitCode)" }
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $handle = $p.MainWindowHandle; break }
  }
  if ($handle -eq [IntPtr]::Zero) { throw "no main window within ${WindowTimeoutSec}s" }
  Write-Host ("smoke: main window 0x{0:X} created" -f $handle.ToInt64())

  Start-Sleep -Seconds $AliveSec
  $p.Refresh()
  if ($p.HasExited) { throw "app exited during smoke window with code $($p.ExitCode)" }
  Write-Host "smoke OK: app stayed alive ${AliveSec}s after window creation"
} finally {
  if (-not $p.HasExited) {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    Write-Host "smoke: app terminated"
  }
}
