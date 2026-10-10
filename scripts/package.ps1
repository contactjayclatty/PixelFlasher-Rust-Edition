$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$binaryPath = Join-Path $projectRoot 'target\release\pixelflasher-rust.exe'
if (-not (Test-Path -LiteralPath $binaryPath)) { throw 'Build the release executable before packaging.' }
$distPath = Join-Path $projectRoot 'dist'
New-Item -ItemType Directory -Path $distPath -Force | Out-Null
Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $distPath 'PixelFlasher.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\pixelflasher-mcp.exe') -Destination (Join-Path $distPath 'pixelflasher-mcp.exe')
foreach ($fileName in @('README.md','CHANGELOG.md','CONTRIBUTING.md','AGENTS.md','LICENSE','LICENSE.upstream','NOTICE.md','THIRD_PARTY_NOTICES.md')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot $fileName) -Destination (Join-Path $distPath $fileName)
}
$distDocs = Join-Path $distPath 'docs'
New-Item -ItemType Directory -Path $distDocs -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $projectRoot 'docs') -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $distDocs $_.Name)
}
$distSkill = Join-Path $distPath '.agents\skills\port-validation'
New-Item -ItemType Directory -Path $distSkill -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot '.agents\skills\port-validation\SKILL.md') -Destination (Join-Path $distSkill 'SKILL.md')
$manifest = [ordered]@{
    name = 'PixelFlasher Rust Edition'
    version = ((Get-Content -LiteralPath (Join-Path $projectRoot 'Cargo.toml') | Select-String '^version = "([^"]+)"').Matches.Groups[1].Value)
    upstreamRevision = '081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54'
    executable = 'PixelFlasher.exe'
    sha256 = (Get-FileHash -LiteralPath (Join-Path $distPath 'PixelFlasher.exe') -Algorithm SHA256).Hash
    mcpExecutable = 'pixelflasher-mcp.exe'
    mcpSha256 = (Get-FileHash -LiteralPath (Join-Path $distPath 'pixelflasher-mcp.exe') -Algorithm SHA256).Hash
    featureParity = 'incomplete; see docs/PARITY.md'
    hardwareValidation = 'not performed'
}
$manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $distPath 'manifest.json') -Encoding utf8
Write-Output (Join-Path $distPath 'PixelFlasher.exe')
