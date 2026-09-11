# =============================================================================
# fetch_engine.ps1 — 按 tool/engine_manifest.json 下载并校验引擎/NNUE/许可证资产
#
# 本地与 CI 共用同一脚本、同一清单：本地验证过的字节 = CI 打包的字节（SHA256 钉死）。
# 重复运行零成本：目标文件已存在且哈希匹配清单时直接跳过。
# 下载顺序：上游官方直链 → 本仓库镜像 release（manifest 的 mirrorUrl，
# 防上游下架/换文件，镜像资产由 .github/workflows/engine-mirror.yml 转存维护）。
# 字体不入本清单：XqKai-Medium-subset.ttf 为入库资产（tool/subset_font.py 生成）。
#
# 用法：
#   ./tool/fetch_engine.ps1                  # 全部资产（core + 各平台引擎）
#   ./tool/fetch_engine.ps1 -Target core     # NNUE + 许可证（跑测试 / 所有平台构建所需）
#   ./tool/fetch_engine.ps1 -Target windows  # core + Windows 引擎
#   ./tool/fetch_engine.ps1 -Target android  # core + Android arm64 引擎
#   ./tool/fetch_engine.ps1 -Target macos    # core + macOS 引擎
#   -Force                                   # 忽略本地已有文件，重新下载/解压
#
# 依赖：7-Zip（Windows: winget install 7zip.7zip；macOS: brew install p7zip；
#       Linux: apt install p7zip-full）。Android x86_64 引擎官方不发布，
#       由 tool/build_android_engine.sh 编译，不经过本脚本。
# =============================================================================
param(
    [ValidateSet('core', 'windows', 'android', 'macos', 'all')]
    [string]$Target = 'all',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $PSScriptRoot 'engine_manifest.json'
$manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json

function Find-7z {
    foreach ($cmd in '7z', '7zz', '7za') {
        $found = Get-Command $cmd -ErrorAction SilentlyContinue
        if ($found) { return $found.Source }
    }
    foreach ($p in @(
        "$env:ProgramFiles\7-Zip\7z.exe",
        "${env:ProgramFiles(x86)}\7-Zip\7z.exe"
    )) {
        if ($p -and (Test-Path $p)) { return $p }
    }
    throw '未找到 7z，请先安装（winget install 7zip.7zip / brew install p7zip / apt install p7zip-full）'
}

$cacheDir = Join-Path ([System.IO.Path]::GetTempPath()) 'pikafish-engine-cache'
New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null

# 主 URL 失败（404/下架/网络异常）时依次尝试镜像 URL；全部失败才报错。
# Invoke-WebRequest 对 4xx/5xx 会抛异常，正常进入回退；命中任一 URL 即返回。
function Save-Url([string[]]$urls, [string]$outFile) {
    $errors = @()
    foreach ($u in ($urls | Where-Object { $_ })) {
        try {
            Write-Host "下载 $u"
            Invoke-WebRequest -Uri $u -OutFile $outFile
            return
        }
        catch {
            $errors += "  $u`n    $($_.Exception.Message)"
            # 下载中途断线可能留下半截文件，清理后试下一个源
            Remove-Item $outFile -Force -ErrorAction SilentlyContinue
        }
    }
    throw "所有下载源均失败：`n$($errors -join "`n")"
}

# 目标 → 资产键清单（许可证随所有目标安装：pubspec 将 NNUE /
# 许可证均声明为 Flutter 资产，任何平台的构建与测试都要求文件在位。
# 字体为入库资产，不经过本脚本）
$targets = @{
    core    = @('nnue', 'license-gpl', 'license-nnue')
    windows = @('nnue', 'windows', 'license-gpl', 'license-nnue')
    android = @('nnue', 'android-arm64', 'license-gpl', 'license-nnue')
    macos   = @('nnue', 'macos', 'license-gpl', 'license-nnue')
    all     = @('nnue', 'windows', 'android-arm64', 'macos',
                'license-gpl', 'license-nnue')
}
$wanted = $targets[$Target]

# 按需解析 7z（core 目标若 NNUE 已就位则无需 7z，延迟到真正要解压时再查找）
$sevenZip = $null
$archive = $null
$extractDir = $null

function Get-Archive {
    # 下载（或复用缓存）钉住版本的官方 7z 总包，校验总包 SHA256，返回解压用的临时目录
    $pk = $manifest.pikafish
    $cachedArchive = Join-Path $cacheDir "$($pk.tag).7z"
    $needDownload = $Force -or -not (Test-Path $cachedArchive)
    if (-not $needDownload) {
        $h = (Get-FileHash $cachedArchive -Algorithm SHA256).Hash
        if ($pk.archiveSha256 -and $h -ne $pk.archiveSha256) {
            Write-Warning "缓存总包哈希不符，重新下载（期望 $($pk.archiveSha256)，实际 $h）"
            $needDownload = $true
        }
    }
    if ($needDownload) {
        Save-Url @($pk.archiveUrl, $pk.mirrorUrl) $cachedArchive
    }
    $h = (Get-FileHash $cachedArchive -Algorithm SHA256).Hash
    Write-Host "总包 SHA256：$h"
    if ($pk.archiveSha256 -and $h -ne $pk.archiveSha256) {
        throw "总包 SHA256 不匹配：期望 $($pk.archiveSha256)，实际 $h"
    }
    $dir = Join-Path $cacheDir 'extract'
    if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    return @($cachedArchive, $dir)
}

function Install-Artifact([string]$key, [pscustomobject]$art) {
    $dest = Join-Path $root $art.dest
    if ((Test-Path $dest) -and -not $Force -and $art.sha256) {
        $have = (Get-FileHash $dest -Algorithm SHA256).Hash
        if ($have -eq $art.sha256) {
            Write-Host "[跳过] $key → $($art.dest)（已是清单版本）"
            return
        }
        Write-Host "[更新] $key（本地哈希与清单不符）"
    }
    elseif ((Test-Path $dest) -and -not $Force) {
        Write-Host "[跳过] $key → $($art.dest)（已存在）"
        return
    }

    # 所有资产均来自 Pikafish 7z 总包的成员
    if (-not $archive) {
        $pair = Get-Archive
        $script:archive = $pair[0]
        $script:extractDir = $pair[1]
        $script:sevenZip = Find-7z
    }
    & $sevenZip e -y "-o$extractDir" $archive $art.member | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "7z 解压失败：$($art.member)（用 7z l 检查包内成员名）" }
    $src = Join-Path $extractDir (Split-Path $art.member -Leaf)

    if ($art.sha256) {
        $h = (Get-FileHash $src -Algorithm SHA256).Hash
        if ($h -ne $art.sha256) { throw "$key SHA256 不匹配：期望 $($art.sha256)，实际 $h" }
    }
    New-Item -ItemType Directory -Force -Path (Split-Path $dest) | Out-Null
    Copy-Item $src $dest -Force
    if ($art.executable -and $PSVersionTable.PSVersion.Major -ge 6 -and ($IsLinux -or $IsMacOS)) {
        chmod +x $dest
    }
    Write-Host "[完成] $key → $($art.dest)"
}

foreach ($key in $wanted) {
    $art = $manifest.pikafish.artifacts.$key
    if (-not $art) { throw "清单中不存在资产键：$key" }
    Install-Artifact $key $art
}
Write-Host "引擎资产就绪（Target=$Target）。"
