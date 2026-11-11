# Hangar probe: collects (masked) launcher metadata for fixtures.
# READ-ONLY on the system; writes only probe-out\ and probe-out.zip in the repo root.
# Run: powershell -ExecutionPolicy Bypass -File scripts\probe.ps1
$ErrorActionPreference = 'SilentlyContinue'

$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root 'probe-out'
$zip = Join-Path $root 'probe-out.zip'
if (Test-Path $out) { Remove-Item $out -Recurse -Force }
if (Test-Path $zip) { Remove-Item $zip -Force }
New-Item -ItemType Directory -Path $out -Force | Out-Null

$utf8 = New-Object System.Text.UTF8Encoding($false)
$summary = New-Object System.Collections.ArrayList

function Log($msg) { [void]$summary.Add($msg); Write-Host $msg }
function Write-Text($path, $text) {
    $dir = Split-Path -Parent $path
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    [IO.File]::WriteAllText($path, $text, $utf8)
}
function Read-Text($path) { [IO.File]::ReadAllText($path, [Text.Encoding]::UTF8) }

# Mask SteamID64 values (17 digits starting 7656119...) with 76561190000000000, incrementing the last digit per distinct id.
function Mask-SteamIds($text) {
    $map = @{}
    $rx = [regex]'7656119\d{10}'
    $ids = @($rx.Matches($text) | ForEach-Object { $_.Value } | Select-Object -Unique)
    $i = 0
    foreach ($id in $ids) {
        $map[$id] = '7656119000000000' + [string]($i % 10)
        $i++
    }
    foreach ($id in $ids) { $text = $text.Replace($id, $map[$id]) }
    return $text
}
function Mask-Keys($text, $keys) {
    foreach ($k in $keys) {
        $text = [regex]::Replace($text, '("' + $k + '"\s+")[^"]*(")', '${1}MASKED${2}')
    }
    return $text
}

# --- Steam ---
$steamPath = (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -Name SteamPath).SteamPath
if (-not $steamPath) {
    Log 'Steam yolu: bulunamadı (HKCU:\Software\Valve\Steam)'
} else {
    $steamPath = $steamPath -replace '/', '\'
    Log "Steam yolu: $steamPath"
    $steamOut = Join-Path $out 'steam'

    $lf = Join-Path $steamPath 'steamapps\libraryfolders.vdf'
    $libPaths = @($steamPath)
    if (Test-Path $lf) {
        $lfText = Read-Text $lf
        Write-Text (Join-Path $steamOut 'libraryfolders.vdf') (Mask-SteamIds $lfText)
        Log 'libraryfolders.vdf: kopyalandı'
        foreach ($m in [regex]::Matches($lfText, '"path"\s+"([^"]*)"')) {
            $p = $m.Groups[1].Value -replace '\\\\', '\'
            if ($libPaths -notcontains $p) { $libPaths += $p }
        }
    } else {
        Log 'libraryfolders.vdf: bulunamadı'
    }

    foreach ($lib in $libPaths) {
        $sa = Join-Path $lib 'steamapps'
        $acfs = @(Get-ChildItem -Path $sa -Filter 'appmanifest_*.acf' -File | Select-Object -First 3)
        if ($acfs.Count -eq 0) { Log "appmanifest: bulunamadı ($sa)"; continue }
        foreach ($a in $acfs) {
            $t = Mask-Keys (Mask-SteamIds (Read-Text $a.FullName)) @('LastOwner')
            Write-Text (Join-Path $steamOut $a.Name) $t
            Log "appmanifest kopyalandı: $($a.Name) ($lib)"
        }
    }

    $lu = Join-Path $steamPath 'config\loginusers.vdf'
    if (Test-Path $lu) {
        $t = Mask-Keys (Mask-SteamIds (Read-Text $lu)) @('AccountName', 'PersonaName')
        Write-Text (Join-Path $steamOut 'loginusers.vdf') $t
        Log 'loginusers.vdf: kopyalandı (maskelendi)'
    } else {
        Log 'loginusers.vdf: bulunamadı'
    }

    $lc = Join-Path $steamPath 'appcache\librarycache'
    if (Test-Path $lc) {
        $listing = New-Object System.Collections.ArrayList
        $dirs = @(Get-ChildItem -Path $lc -Directory | Select-Object -First 2)
        if ($dirs.Count -gt 0) {
            foreach ($d in $dirs) {
                [void]$listing.Add("[$($d.Name)]")
                Get-ChildItem -Path $d.FullName -Recurse -File | ForEach-Object { [void]$listing.Add($_.FullName.Substring($d.FullName.Length).TrimStart('\')) }
            }
        } else {
            # Older layout: flat files named <appid>_*.jpg
            [void]$listing.Add('(alt klasör yok, düz dosya listesi, ilk 20)')
            Get-ChildItem -Path $lc -File | Select-Object -First 20 | ForEach-Object { [void]$listing.Add($_.Name) }
        }
        Write-Text (Join-Path $steamOut 'librarycache-listing.txt') ($listing -join "`r`n")
        Log 'librarycache: dosya listesi yazıldı'
    } else {
        Log 'librarycache: bulunamadı'
    }
}

# --- Epic ---
$epicDir = 'C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests'
$items = @(Get-ChildItem -Path $epicDir -Filter '*.item' -File | Select-Object -First 3)
if ($items.Count -eq 0) {
    Log "Epic manifest: bulunamadı ($epicDir)"
} else {
    foreach ($it in $items) {
        Write-Text (Join-Path $out ('epic\' + $it.Name)) (Read-Text $it.FullName)
        Log "Epic manifest kopyalandı: $($it.Name)"
    }
}

# --- Ubisoft ---
$ubiKey = 'HKLM:\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs'
$subs = @(Get-ChildItem -Path $ubiKey)
if ($subs.Count -eq 0) {
    Log "Ubisoft Installs: bulunamadı ($ubiKey)"
} else {
    $rows = @()
    foreach ($s in $subs) {
        $dir = (Get-ItemProperty -Path $s.PSPath -Name InstallDir).InstallDir
        $rows += @{ id = $s.PSChildName; install_dir = $dir }
    }
    Write-Text (Join-Path $out 'ubisoft\installs.json') (ConvertTo-Json -InputObject @($rows))
    Log "Ubisoft Installs: $($rows.Count) kayıt yazıldı"
}

Write-Text (Join-Path $out 'summary.txt') ($summary -join "`r`n")
Compress-Archive -Path (Join-Path $out '*') -DestinationPath $zip -Force
Log "Tamam: $zip"
Write-Host 'Lütfen probe-out.zip dosyasını paylaşın.'
