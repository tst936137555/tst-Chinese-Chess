param([string]$Toolchain, [string]$Python, [switch]$WithEngine, [switch]$DoctorOnly)
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $cargoBin = Join-Path $env:USERPROFILE '.cargo/bin'
        if (Test-Path -LiteralPath (Join-Path $cargoBin 'cargo.exe')) { $env:PATH = "$cargoBin;$env:PATH" }
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw '未找到 Cargo，请安装 Rust 或将 .cargo/bin 加入 PATH。' }
    if (-not $Toolchain) {
        $channel = [regex]::Match((Get-Content rust-toolchain.toml -Raw), 'channel\s*=\s*"([^"]+)"').Groups[1].Value
        $Toolchain = if ($IsWindows -and -not (Get-Command link.exe -ErrorAction SilentlyContinue) -and (Get-Command gcc -ErrorAction SilentlyContinue)) { "$channel-x86_64-pc-windows-gnu" } else { $channel }
    }
    if ($Toolchain -like '*windows-gnu' -and -not (Get-Command windres -ErrorAction SilentlyContinue)) { throw 'GNU 工具链需要 GCC 和 windres，请安装 MinGW 并加入 PATH。' }
    if (-not $Python) {
        foreach ($name in @('python3','python')) {
            $candidate = Get-Command $name -ErrorAction SilentlyContinue
            if ($candidate -and $candidate.Source -notmatch 'WindowsApps') { $Python = $candidate.Source; break }
        }
    }
    if (-not $Python -or -not (Test-Path -LiteralPath $Python)) { throw '未找到 Python 3.11+。请用 -Python 传入实际解释器路径。' }
    & $Python -c 'import sys; assert sys.version_info >= (3,11), "Python 3.11+ required"'
    if ($LASTEXITCODE -ne 0) { throw 'Python 版本不符合要求。' }
    & rustc "+$Toolchain" --version
    if ($LASTEXITCODE -ne 0) { throw "Rust 工具链不可用：$Toolchain" }
    Write-Host "Toolchain: $Toolchain"
    Write-Host "Python: $Python"
    if ($DoctorOnly) { return }
    if ($WithEngine) {
        $target = if ($IsMacOS) { 'macos' } elseif ($IsWindows) { 'windows' } else { 'core' }
        & "$PSScriptRoot/fetch_engine.ps1" -Target $target
        if ($IsWindows -or $IsMacOS) { $env:XIANGQI_REQUIRE_ENGINE = '1' }
    }
    & cargo "+$Toolchain" fmt --check
    if ($LASTEXITCODE -ne 0) { throw '格式检查失败。' }
    & cargo "+$Toolchain" clippy --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy 检查失败。' }
    & cargo "+$Toolchain" test --all-targets --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust 回归失败。' }
    foreach ($pattern in @('test_*smoke.py','test_android_bridge.py')) {
        & $Python -m unittest discover -s tool -p $pattern
        if ($LASTEXITCODE -ne 0) { throw "Python 回归失败：$pattern" }
    }
    & "$PSScriptRoot/test_download_fallback.ps1"
} finally { Pop-Location }
