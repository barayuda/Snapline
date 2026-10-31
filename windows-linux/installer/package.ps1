# Builds the Windows release files from target\release\snapline.exe: the installer, the portable zip and
# SHA256SUMS.txt. CI runs it on every push and the release workflow runs it on a tag, so a release is built
# exactly as CI tests it. -SumsOnly writes SHA256SUMS.txt again (after the installer has been signed).
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Out = "target\dist",
    [switch]$SumsOnly
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path "$PSScriptRoot\..\.."
$exe = Resolve-Path "$PSScriptRoot\..\target\release\snapline.exe"
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = Resolve-Path $Out

if (-not $SumsOnly) {
    $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    if (Test-Path $iscc) { "Inno Setup: preinstalled" } else { "Inno Setup: installing"; choco install innosetup --no-progress -y }
    & $iscc /Qp "/DVersion=$Version" "/DSource=$exe" "/O$Out" "$PSScriptRoot\snapline.iss"
    if ($LASTEXITCODE) { throw "ISCC failed with code $LASTEXITCODE" }
    $zip = Join-Path $Out "Snapline-$Version-windows-x64.zip"
    Compress-Archive -Force -Path $exe, "$root\LICENSE.md", "$root\README.md" -DestinationPath $zip
}

# "<sha256>  <file>", as sha256sum writes it, so `sha256sum -c SHA256SUMS.txt` checks the downloads.
$sums = Get-ChildItem $Out -File | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
}
Set-Content -Path (Join-Path $Out 'SHA256SUMS.txt') -Value $sums -Encoding ascii
Get-ChildItem $Out | Format-Table Name, Length
