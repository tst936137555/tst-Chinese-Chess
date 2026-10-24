param([string]$Toolchain)
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    if (-not $Toolchain) {
        $Toolchain = [regex]::Match((Get-Content rust-toolchain.toml -Raw), 'channel\s*=\s*"([^"]+)"').Groups[1].Value
        if (-not $Toolchain) { throw 'rust-toolchain.toml 缺少 channel' }
    }
    & "$PSScriptRoot/fetch_engine.ps1" -Target windows
    $packageTarget = ((rustc "+$Toolchain" -vV | Select-String '^host: ').Line -replace '^host: ', '').Trim()
    if (-not $packageTarget) { throw '无法识别构建目标' }
    cargo "+$Toolchain" build --release --locked --target $packageTarget
    if ($LASTEXITCODE -ne 0) { throw 'Rust 构建失败' }
    $meta = cargo "+$Toolchain" metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw '读取版本失败' }
    $version = ($meta.packages | Where-Object { $_.name -eq 'tst-xiangqi' }).version
    $distRoot = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) 'dist'))
    New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
    $finalPackage = Join-Path $distRoot "tst-xiangqi-$version-windows"
    $package = Join-Path (Get-Location) ("target/package-staging/" + [guid]::NewGuid().ToString('N') + "/tst-xiangqi-$version-windows")
    New-Item -ItemType Directory -Force -Path "$package/assets/engine", "$package/assets/licenses", "$package/assets/fonts", "$package/readme" | Out-Null
    $packageBinary = Join-Path $meta.target_directory "$packageTarget/release/tst-xiangqi.exe"
    Copy-Item -LiteralPath $packageBinary -Destination $package -Force
    Copy-Item -LiteralPath 'pikafish.exe' -Destination "$package/assets/engine/" -Force
    Copy-Item -LiteralPath 'LICENSE' -Destination "$package/assets/licenses/" -Force
    Copy-Item -LiteralPath 'CHANGELOG.md' -Destination "$package/readme/" -Force
    # Rebase local Markdown links to the complete source tree beside this manual.
    $manual = [System.IO.File]::ReadAllText((Join-Path (Get-Location) 'README.md'))
    $manual = [regex]::Replace($manual, '\]\((?![a-zA-Z][a-zA-Z0-9+.-]*:|#|/)([^)]+)\)', '](../source/$1)')
    [System.IO.File]::WriteAllText((Join-Path $package 'readme/README.md'), $manual, [System.Text.UTF8Encoding]::new($false))
    Copy-Item -LiteralPath 'assets/engine/pikafish.nnue' -Destination "$package/assets/engine/" -Force
    Copy-Item -Path 'assets/licenses/*' -Destination "$package/assets/licenses/" -Force
    Copy-Item -LiteralPath 'assets/fonts/OFL.txt' -Destination "$package/assets/fonts/" -Force
    Copy-Item -LiteralPath 'vendor/quad-snd/LICENSE' -Destination "$package/assets/licenses/quad-snd-MIT.txt" -Force
    Copy-Item -LiteralPath 'vendor/miniquad/LICENSE-MIT' -Destination "$package/assets/licenses/miniquad-MIT.txt" -Force
    Copy-Item -LiteralPath 'vendor/miniquad/LICENSE-APACHE' -Destination "$package/assets/licenses/miniquad-Apache-2.0.txt" -Force
    $sources = Join-Path $package 'source'
    New-Item -ItemType Directory -Force -Path $sources | Out-Null
    Copy-Item -LiteralPath 'Cargo.toml', 'Cargo.lock', 'build.rs', 'rust-toolchain.toml', 'LICENSE', 'README.md', 'CHANGELOG.md' -Destination $sources -Force
    foreach ($directory in @('.github','docs','src','tests','examples','native','tool','vendor','assets/icons','assets/fonts','assets/licenses')) {
        Get-ChildItem -LiteralPath $directory -File -Recurse | Where-Object {
            $_.FullName -notmatch '[\\/](target|__pycache__|[.]git)[\\/]' -and $_.Extension -ne '.pyc'
        } | ForEach-Object {
            $relative = [System.IO.Path]::GetRelativePath((Get-Location).Path, $_.FullName)
            $destination = Join-Path $sources $relative
            New-Item -ItemType Directory -Force -Path (Split-Path $destination) | Out-Null
            Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
        }
    }
    Push-Location $env:TEMP
    try {
        $process = Start-Process -FilePath (Join-Path $package 'tst-xiangqi.exe') -ArgumentList '--self-test' -WindowStyle Hidden -PassThru
        if (-not $process.WaitForExit(45000)) { $process.Kill(); throw '便携包冒烟测试超时' }
        if ($process.ExitCode -ne 0) { throw '便携包真实引擎/续局冒烟测试失败' }
    } finally { Pop-Location }
    if (@(Get-ChildItem -LiteralPath $package -File).Count -ne 1) { throw '包根目录必须只包含游戏 EXE' }
    python tool/release_manifest.py $package --toolchain $Toolchain --target $packageTarget --output metadata/BUILD_INFO.json --build-command "cargo +$Toolchain build --release --locked --target $packageTarget"
    if ($LASTEXITCODE -ne 0) { throw '生成发布清单失败' }
    $resolvedFinal = [System.IO.Path]::GetFullPath($finalPackage)
    if (-not $resolvedFinal.StartsWith($distRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) { throw '打包目标越出 dist 目录' }
    if (Test-Path -LiteralPath $resolvedFinal) { Remove-Item -LiteralPath $resolvedFinal -Recurse -Force }
    Move-Item -LiteralPath $package -Destination $resolvedFinal
    Compress-Archive -Path $resolvedFinal -DestinationPath "$resolvedFinal.zip" -Force
    Write-Host "已生成 $resolvedFinal.zip"
}
finally { Pop-Location }
