# Installs the fratm CLI on Windows from the latest GitHub release.
#
#   irm https://raw.githubusercontent.com/kkratterf/fratmscript/main/install.ps1 | iex
#
# Environment:
#   FRATM_VERSION      release to install, e.g. v0.5.0 (default: latest)
#   FRATM_INSTALL_DIR  where to put fratm.exe (default: $HOME\.fratm\bin)

$ErrorActionPreference = 'Stop'

$Repo = 'kkratterf/fratmscript'
$Target = 'x86_64-pc-windows-msvc'
$InstallDir = if ($env:FRATM_INSTALL_DIR) { $env:FRATM_INSTALL_DIR } else { Join-Path $HOME '.fratm\bin' }

$Version = $env:FRATM_VERSION
if (-not $Version) {
    # A token (as in CI) avoids the API rate limit for anonymous requests
    $Headers = @{}
    if ($env:GH_TOKEN) { $Headers['Authorization'] = "Bearer $env:GH_TOKEN" }
    $Version = (Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers $Headers).tag_name
}
if (-not $Version.StartsWith('v')) { $Version = "v$Version" }

$Name = "fratm-$Version-$Target"
$Base = "https://github.com/$Repo/releases/download/$Version"
$Tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $Tmp | Out-Null

try {
    Write-Host "Scarico fratm $Version ($Target)..."
    $Zip = Join-Path $Tmp "$Name.zip"
    Invoke-WebRequest "$Base/$Name.zip" -OutFile $Zip -UseBasicParsing

    $Sums = Join-Path $Tmp 'SHA256SUMS.txt'
    Invoke-WebRequest "$Base/SHA256SUMS.txt" -OutFile $Sums -UseBasicParsing
    $Expected = (Select-String -Path $Sums -Pattern " $([regex]::Escape("$Name.zip"))$").Line.Split(' ')[0]
    $Actual = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLower()
    if ($Expected -ne $Actual) { throw "checksum sbagliato pe $Name.zip" }

    Expand-Archive $Zip -DestinationPath $Tmp
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $Tmp "$Name\fratm.exe") (Join-Path $InstallDir 'fratm.exe') -Force
} finally {
    Remove-Item -Recurse -Force $Tmp
}

Write-Host "Tutto appost! fratm installato in $InstallDir\fratm.exe"
$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not ($UserPath -split ';' | Where-Object { $_ -eq $InstallDir })) {
    [Environment]::SetEnvironmentVariable('Path', "$UserPath;$InstallDir", 'User')
    Write-Host "Aggiunto $InstallDir al PATH: apri un terminale nuovo."
}
