# 合成象棋音效：落子/吃子/将军/胜利/失败/和棋（44100Hz 16bit 双声道 WAV）
# 用法：在项目根目录执行  & .\tool\gen_sounds.ps1
$sr = 44100
$rng = New-Object System.Random(7)

function Write-Wav([string]$name, [double[]]$samples) {
    # 双声道交错写入（L/R 相同）
    $dataLen = $samples.Length * 2 * 2
    $ms = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter($ms)
    $ascii = [System.Text.Encoding]::ASCII
    $bw.Write($ascii.GetBytes("RIFF"))
    $bw.Write([int](36 + $dataLen))
    $bw.Write($ascii.GetBytes("WAVE"))
    $bw.Write($ascii.GetBytes("fmt "))
    $bw.Write([int]16)
    $bw.Write([int16]1)      # PCM
    $bw.Write([int16]2)      # 双声道
    $bw.Write([int]$sr)
    $bw.Write([int]($sr * 2 * 2))
    $bw.Write([int16]4)      # 块对齐（采样宽 x 声道）
    $bw.Write([int16]16)
    $bw.Write($ascii.GetBytes("data"))
    $bw.Write([int]$dataLen)
    foreach ($s in $samples) {
        $v = [int]([Math]::Max(-1.0, [Math]::Min(1.0, $s)) * 32767)
        $bw.Write([int16]$v)  # 左声道
        $bw.Write([int16]$v)  # 右声道
    }
    $bw.Flush()
    New-Item -ItemType Directory -Force -Path "assets\sounds" | Out-Null
    [System.IO.File]::WriteAllBytes((Join-Path "assets\sounds" $name), $ms.ToArray())
    Write-Output "$name $([math]::Round($samples.Length / $sr, 2))s"
}

# ADSR 包络
function Get-Adsr([int]$n, [double]$a, [double]$d, [double]$s, [double]$r) {
    $env = New-Object double[] $n
    $na = [int]($a * $sr); $nd = [int]($d * $sr); $nr = [int]($r * $sr)
    $ns = [Math]::Max(0, $n - $na - $nd - $nr)
    for ($i = 0; $i -lt $na -and $i -lt $n; $i++) { $env[$i] = $i / [Math]::Max(1, $na) }
    for ($i = 0; $i -lt $nd -and ($na + $i) -lt $n; $i++) { $env[$na + $i] = 1 - (1 - $s) * $i / [Math]::Max(1, $nd) }
    for ($i = 0; $i -lt $ns -and ($na + $nd + $i) -lt $n; $i++) { $env[$na + $nd + $i] = $s }
    for ($i = 0; $i -lt $nr; $i++) {
        $idx = $n - $nr + $i
        if ($idx -ge 0 -and $idx -lt $n) { $env[$idx] = $s * (1 - $i / [Math]::Max(1, $nr)) }
    }
    return ,$env
}

# 落子：短促的木质敲击（低频正弦 + 快速衰减 + 轻微高频瞬态）
function Get-Place {
    $n = [int](0.09 * $sr)
    $out = New-Object double[] $n
    for ($i = 0; $i -lt $n; $i++) {
        $t = $i / $sr
        $env = [Math]::Exp(-$t * 55)
        $v = [Math]::Sin(2 * [Math]::PI * 190 * $t) * 0.9
        $v += [Math]::Sin(2 * [Math]::PI * 380 * $t) * 0.35 * [Math]::Exp(-$t * 90)
        if ($t -lt 0.004) {
            $v += ($rng.NextDouble() * 2 - 1) * 0.5 * (1 - $t / 0.004)
        }
        $out[$i] = $v * $env * 0.85
    }
    return ,$out
}

# 吃子：双击 + 更厚重的低频
function Get-Capture {
    $out = New-Object 'System.Collections.Generic.List[double]'
    foreach ($f in @(165.0, 140.0)) {
        $n = [int](0.1 * $sr)
        for ($i = 0; $i -lt $n; $i++) {
            $t = $i / $sr
            $env = [Math]::Exp(-$t * 45)
            $v = [Math]::Sin(2 * [Math]::PI * $f * $t) * 0.95
            $v += [Math]::Sin(2 * [Math]::PI * $f * 2 * $t) * 0.4 * [Math]::Exp(-$t * 80)
            if ($t -lt 0.003) {
                $v += ($rng.NextDouble() * 2 - 1) * 0.6 * (1 - $t / 0.003)
            }
            $out.Add($v * $env * 0.9)
        }
    }
    return ,$out.ToArray()
}

# 将军：两声急促的中频提示
function Get-Check {
    $out = New-Object 'System.Collections.Generic.List[double]'
    foreach ($f in @(660.0, 880.0)) {
        $n = [int](0.09 * $sr)
        for ($i = 0; $i -lt $n; $i++) {
            $t = $i / $sr
            $env = [Math]::Exp(-$t * 30) * (1 - [Math]::Exp(-$t * 400))
            $v = [Math]::Sin(2 * [Math]::PI * $f * $t) + 0.3 * [Math]::Sin(2 * [Math]::PI * $f * 2 * $t)
            $out.Add($v * $env * 0.6)
        }
        for ($i = 0; $i -lt [int](0.035 * $sr); $i++) { $out.Add(0.0) }
    }
    return ,$out.ToArray()
}

# 胜利：上行琶音 C-E-G-C
function Get-Win {
    $out = New-Object 'System.Collections.Generic.List[double]'
    $freqs = @(523.0, 659.0, 784.0, 1047.0)
    for ($k = 0; $k -lt $freqs.Count; $k++) {
        $f = $freqs[$k]
        $dur = if ($k -lt 3) { 0.16 } else { 0.5 }
        $n = [int]($dur * $sr)
        $env = Get-Adsr $n 0.005 0.08 0.5 0.15
        for ($i = 0; $i -lt $n; $i++) {
            $t = $i / $sr
            $v = [Math]::Sin(2 * [Math]::PI * $f * $t) + 0.35 * [Math]::Sin(2 * [Math]::PI * $f * 2 * $t)
            $out.Add($v * $env[$i] * 0.55)
        }
        for ($i = 0; $i -lt [int](0.02 * $sr); $i++) { $out.Add(0.0) }
    }
    return ,$out.ToArray()
}

# 失败：下行三音
function Get-Lose {
    $out = New-Object 'System.Collections.Generic.List[double]'
    $freqs = @(392.0, 330.0, 262.0)
    for ($k = 0; $k -lt $freqs.Count; $k++) {
        $f = $freqs[$k]
        $dur = if ($k -eq 2) { 0.5 } else { 0.22 }
        $n = [int]($dur * $sr)
        $env = Get-Adsr $n 0.005 0.1 0.45 0.2
        for ($i = 0; $i -lt $n; $i++) {
            $t = $i / $sr
            $v = [Math]::Sin(2 * [Math]::PI * $f * $t) + 0.3 * [Math]::Sin(2 * [Math]::PI * $f * 2 * $t)
            $out.Add($v * $env[$i] * 0.55)
        }
        for ($i = 0; $i -lt [int](0.03 * $sr); $i++) { $out.Add(0.0) }
    }
    return ,$out.ToArray()
}

# 和棋：两声中性单音
function Get-Draw {
    $out = New-Object 'System.Collections.Generic.List[double]'
    for ($k = 0; $k -lt 2; $k++) {
        $n = [int](0.25 * $sr)
        $env = Get-Adsr $n 0.005 0.08 0.4 0.12
        for ($i = 0; $i -lt $n; $i++) {
            $t = $i / $sr
            $v = [Math]::Sin(2 * [Math]::PI * 440 * $t) + 0.25 * [Math]::Sin(2 * [Math]::PI * 440 * 2 * $t)
            $out.Add($v * $env[$i] * 0.5)
        }
        for ($i = 0; $i -lt [int](0.06 * $sr); $i++) { $out.Add(0.0) }
    }
    return ,$out.ToArray()
}

Write-Wav "place.wav" (Get-Place)
Write-Wav "capture.wav" (Get-Capture)
Write-Wav "check.wav" (Get-Check)
Write-Wav "win.wav" (Get-Win)
Write-Wav "lose.wav" (Get-Lose)
Write-Wav "draw.wav" (Get-Draw)
Write-Output "done"
