$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tokens = $null; $parseErrors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'fetch_engine.ps1'), [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw 'Download script parse failed' }
$functionAst = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Save-Url' }, $true)
Invoke-Expression $functionAst.Extent.Text
$testRoot = Join-Path $root ('target/download-regression-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$script:mode = 'corrupt'
$script:calls = [System.Collections.Generic.List[string]]::new()
function Invoke-WebRequest([string]$Uri, [string]$OutFile) {
    $script:calls.Add($Uri)
    if ($script:mode -eq 'network' -and $Uri -eq 'primary') { throw 'simulated connection failure' }
    $value = if ($Uri -eq 'mirror' -and $script:mode -ne 'both-corrupt') { 'verified bytes' } else { 'wrong bytes' }
    [System.IO.File]::WriteAllText($OutFile, $value, [System.Text.UTF8Encoding]::new($false))
}
$out = Join-Path $testRoot 'asset.bin'
[System.IO.File]::WriteAllText($out, 'verified bytes', [System.Text.UTF8Encoding]::new($false))
$sha = (Get-FileHash -LiteralPath $out).Hash
foreach ($scenario in @('corrupt','network')) {
    $script:mode = $scenario; $script:calls.Clear()
    Save-Url @('primary','mirror') $out $sha
    if ((Get-FileHash -LiteralPath $out).Hash -ne $sha -or ($script:calls -join ',') -ne 'primary,mirror') { throw "$scenario fallback failed" }
}
$script:mode = 'both-corrupt'
$failed = $false
try { Save-Url @('primary','mirror') $out $sha } catch { $failed = $true }
if (-not $failed -or (Test-Path -LiteralPath $out)) { throw 'Corrupt downloads were accepted or retained' }
Write-Host 'Download SHA256/network fallback regression passed'

