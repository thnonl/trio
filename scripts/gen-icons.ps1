# Generates Trio's app icons (PNG + ICO + ICNS) with no external tooling.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/gen-icons.ps1
# Output: src-tauri/icons/

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root "src-tauri\icons"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function New-RoundedRectPath {
    param([float]$X, [float]$Y, [float]$W, [float]$H, [float]$R)
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $R * 2
    $path.AddArc($X, $Y, $d, $d, 180, 90)
    $path.AddArc($X + $W - $d, $Y, $d, $d, 270, 90)
    $path.AddArc($X + $W - $d, $Y + $H - $d, $d, $d, 0, 90)
    $path.AddArc($X, $Y + $H - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    return $path
}

function New-TrioBitmap {
    param([int]$Size)

    # Always draw at 512 and resample, so every size shares identical geometry.
    $canvas = 512
    $bmp = New-Object System.Drawing.Bitmap($canvas, $canvas)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    $bg = New-RoundedRectPath -X 8 -Y 8 -W 496 -H 496 -R 112
    $bgBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        (New-Object System.Drawing.Point(0, 0)),
        (New-Object System.Drawing.Point(0, 512)),
        [System.Drawing.Color]::FromArgb(255, 26, 29, 42),
        [System.Drawing.Color]::FromArgb(255, 14, 16, 24)
    )
    $g.FillPath($bgBrush, $bg)

    $rim = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(46, 255, 255, 255), 3)
    $g.DrawPath($rim, $bg)

    $bars = @(
        @{ X = 148; H = 150; Color = [System.Drawing.Color]::FromArgb(255, 94, 104, 214) },
        @{ X = 235; H = 232; Color = [System.Drawing.Color]::FromArgb(255, 124, 134, 255) },
        @{ X = 322; H = 190; Color = [System.Drawing.Color]::FromArgb(255, 154, 162, 255) }
    )

    $bottom = 366
    $width = 42
    foreach ($bar in $bars) {
        $rect = New-RoundedRectPath -X $bar.X -Y ($bottom - $bar.H) -W $width -H $bar.H -R ($width / 2)
        $brush = New-Object System.Drawing.SolidBrush($bar.Color)
        $g.FillPath($brush, $rect)
        $brush.Dispose()
        $rect.Dispose()
    }

    $bgBrush.Dispose()
    $rim.Dispose()
    $bg.Dispose()
    $g.Dispose()

    if ($Size -ne 512) {
        $scaled = New-Object System.Drawing.Bitmap($Size, $Size)
        $sg = [System.Drawing.Graphics]::FromImage($scaled)
        $sg.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $sg.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $sg.DrawImage($bmp, 0, 0, $Size, $Size)
        $sg.Dispose()
        $bmp.Dispose()
        return $scaled
    }

    return $bmp
}

function Save-Png {
    param([int]$Size, [string]$FileName)
    $bmp = New-TrioBitmap -Size $Size
    $path = Join-Path $outDir $FileName
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "wrote $FileName ($Size px)"
}

# PNGs referenced by tauri.conf.json
Save-Png -Size 32 -FileName "32x32.png"
Save-Png -Size 128 -FileName "128x128.png"
Save-Png -Size 256 -FileName "128x128@2x.png"
Save-Png -Size 512 -FileName "icon.png"
Save-Png -Size 1024 -FileName "icon-1024.png"

function Get-PngBytes {
    param([int]$Size)
    $bmp = New-TrioBitmap -Size $Size
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $bytes = $ms.ToArray()
    $ms.Dispose()
    return , $bytes
}

# --- ICO (PNG-compressed, Vista+) ---
$icoPng = Get-PngBytes -Size 256
$ico = New-Object System.Collections.Generic.List[byte]
$ico.AddRange([byte[]]@(0, 0, 1, 0, 1, 0))                       # reserved, type=icon, count=1
$ico.AddRange([byte[]]@(0, 0, 0, 0, 1, 0, 32, 0))                # 256x256, no palette, 1 plane, 32bpp
$ico.AddRange([System.BitConverter]::GetBytes([uint32]$icoPng.Length))
$ico.AddRange([System.BitConverter]::GetBytes([uint32]22))       # offset
$ico.AddRange($icoPng)
[System.IO.File]::WriteAllBytes((Join-Path $outDir "icon.ico"), $ico.ToArray())
Write-Host "wrote icon.ico"

# --- ICNS (PNG entries; accepted by macOS 10.7+) ---
$icnsTypes = [ordered]@{
    "icp4" = 16
    "ic11" = 32
    "icp5" = 32
    "ic12" = 64
    "icp6" = 64
    "ic07" = 128
    "ic13" = 256
    "ic08" = 256
    "ic14" = 512
    "ic09" = 512
    "ic10" = 1024
}

$cache = @{}
$body = New-Object System.Collections.Generic.List[byte]
foreach ($type in $icnsTypes.Keys) {
    $size = $icnsTypes[$type]
    if (-not $cache.ContainsKey($size)) {
        $cache[$size] = Get-PngBytes -Size $size
    }
    $png = $cache[$size]
    $body.AddRange([System.Text.Encoding]::ASCII.GetBytes($type))
    $body.AddRange([System.BitConverter]::GetBytes([uint32]($png.Length + 8)))
    $body.AddRange($png)
}

$header = New-Object System.Collections.Generic.List[byte]
$header.AddRange([System.Text.Encoding]::ASCII.GetBytes("icns"))
$header.AddRange([System.BitConverter]::GetBytes([uint32]($body.Count + 8)))
$header.AddRange($body)
[System.IO.File]::WriteAllBytes((Join-Path $outDir "icon.icns"), $header.ToArray())
Write-Host "wrote icon.icns"

Get-ChildItem $outDir | Select-Object Name, Length | Format-Table -AutoSize
