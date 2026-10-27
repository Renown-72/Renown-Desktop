# 一键打包发行版：安装包 + 便携版 + 校验文件
#
# 用法（在仓库根，即 app/ 目录执行）：
#   pwsh -File scripts/build-release.ps1
#   pwsh -File scripts/build-release.ps1 -OutDir ..\dist
#
# 背景：本机 hosts 把 github.com 指到 127.0.0.1（广告屏蔽列表所为），
# 而 tauri 打包时要从 github.com 下载 NSIS 工具链，直连必然失败。
# 这个脚本会自动把 NSIS 预置到 tauri 缓存目录，绕开下载。

[CmdletBinding()]
param(
    [string]$OutDir = (Join-Path $PSScriptRoot '..\..\dist'),
    [switch]$SkipToolchainCheck
)

$ErrorActionPreference = 'Stop'
$appRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$OutDir = [System.IO.Path]::GetFullPath($OutDir)

Write-Host "== Renown 发行打包 ==" -ForegroundColor Cyan
Write-Host "   仓库: $appRoot"
Write-Host "   输出: $OutDir"

# ---------------------------------------------------------------------------
# 1. 确保 NSIS 工具链就绪（绕开被 hosts 屏蔽的 github.com）
# ---------------------------------------------------------------------------
$nsisCache = Join-Path $env:LOCALAPPDATA 'tauri\NSIS'
$required = @(
    'makensis.exe',
    'Bin\makensis.exe',
    'Include\MUI2.nsh',
    'Include\FileFunc.nsh',
    'Include\x64.nsh',
    'Include\nsDialogs.nsh',
    'Include\WinMessages.nsh',
    'Plugins\x86-unicode\additional\nsis_tauri_utils.dll'
)

function Test-NsisReady {
    foreach ($rel in $required) {
        if (-not (Test-Path (Join-Path $nsisCache $rel))) { return $false }
    }
    return $true
}

function Invoke-GithubDownload {
    param([string]$Url, [string]$OutFile)
    # 依次尝试几个 github.com 的真实 IP，绕过 hosts 污染
    $ips = @('140.82.114.4', '140.82.113.4', '140.82.112.4', '140.82.121.4')
    foreach ($ip in $ips) {
        Write-Host "   尝试 $ip ..."
        $args = @(
            '-sSL', '--max-time', '240',
            '--resolve', "github.com:443:$ip",
            '--resolve', 'objects.githubusercontent.com:443:185.199.108.133',
            '--resolve', 'release-assets.githubusercontent.com:443:185.199.108.133',
            '-o', $OutFile, $Url
        )
        & curl.exe @args
        if ((Test-Path $OutFile) -and (Get-Item $OutFile).Length -gt 10000) {
            return $true
        }
        Remove-Item $OutFile -Force -ErrorAction SilentlyContinue
    }
    return $false
}

if (-not $SkipToolchainCheck -and -not (Test-NsisReady)) {
    Write-Host '   NSIS 工具链不完整，正在预置...' -ForegroundColor Yellow
    $tmp = Join-Path $env:TEMP 'renown-nsis'
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null

    $zip = Join-Path $tmp 'nsis-3.11.zip'
    if (-not (Test-Path $zip)) {
        Write-Host '   下载 nsis-3.11.zip'
        if (-not (Invoke-GithubDownload 'https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip' $zip)) {
            throw '下载 NSIS 失败。请检查网络，或先给 FlClash 开启代理后重试。'
        }
    }

    $extract = Join-Path $tmp 'extract'
    if (Test-Path $extract) { Remove-Item $extract -Recurse -Force }
    Expand-Archive -Path $zip -DestinationPath $extract -Force
    $inner = Get-ChildItem $extract -Directory | Select-Object -First 1
    New-Item -ItemType Directory -Force -Path $nsisCache | Out-Null
    Copy-Item (Join-Path $inner.FullName '*') -Destination $nsisCache -Recurse -Force
    Write-Host "   NSIS 已铺到 $nsisCache"

    $dllDest = Join-Path $nsisCache 'Plugins\x86-unicode\additional'
    New-Item -ItemType Directory -Force -Path $dllDest | Out-Null
    $dll = Join-Path $tmp 'nsis_tauri_utils.dll'
    if (-not (Test-Path $dll)) {
        Write-Host '   下载 nsis_tauri_utils.dll'
        if (-not (Invoke-GithubDownload 'https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll' $dll)) {
            throw '下载 nsis_tauri_utils.dll 失败。'
        }
    }
    Copy-Item $dll (Join-Path $dllDest 'nsis_tauri_utils.dll') -Force

    if (-not (Test-NsisReady)) { throw 'NSIS 预置后校验仍未通过。' }
    Write-Host '   NSIS 工具链就绪' -ForegroundColor Green
}

# ---------------------------------------------------------------------------
# 2. 打包
# ---------------------------------------------------------------------------
Push-Location $appRoot
try {
    Write-Host '   运行 tauri build（含 NSIS 安装包）...' -ForegroundColor Cyan
    & npx tauri build
    if ($LASTEXITCODE -ne 0) { throw "tauri build 失败（exit $LASTEXITCODE）" }
}
finally {
    Pop-Location
}

$version = (Get-Content (Join-Path $appRoot 'package.json') -Raw | ConvertFrom-Json).version
$setup = Join-Path $appRoot "src-tauri\target\release\bundle\nsis\Renown_${version}_x64-setup.exe"
$portableSrc = Join-Path $appRoot 'src-tauri\target\release\renown.exe'
if (-not (Test-Path $setup)) { throw "找不到安装包：$setup" }
if (-not (Test-Path $portableSrc)) { throw "找不到便携版 exe：$portableSrc" }

# ---------------------------------------------------------------------------
# 3. 组装发行目录
# ---------------------------------------------------------------------------
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Get-ChildItem $OutDir -File | Remove-Item -Force -ErrorAction SilentlyContinue

$setupOut = Join-Path $OutDir "Renown_${version}_x64-setup.exe"
$portableOut = Join-Path $OutDir "Renown-Portable-${version}-x64.exe"
Copy-Item $setup $setupOut -Force
Copy-Item $portableSrc $portableOut -Force

$readmeSrc = Join-Path $PSScriptRoot 'RELEASE-README.txt'
if (Test-Path $readmeSrc) {
    Copy-Item $readmeSrc (Join-Path $OutDir 'README.txt') -Force
}

$lines = Get-ChildItem $OutDir -File | Where-Object { $_.Name -ne 'SHA256SUMS.txt' } | ForEach-Object {
    $h = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower()
    "$h  $($_.Name)"
}
Set-Content -Path (Join-Path $OutDir 'SHA256SUMS.txt') -Value ($lines -join "`n") -Encoding ASCII

# 再打一个整包 zip，方便直接分发
$zipOut = Join-Path (Split-Path $OutDir -Parent) "Renown-${version}-x64.zip"
if (Test-Path $zipOut) { Remove-Item $zipOut -Force }
Compress-Archive -Path (Join-Path $OutDir '*') -DestinationPath $zipOut -CompressionLevel Optimal

Write-Host ''
Write-Host '== 完成 ==' -ForegroundColor Green
Get-ChildItem $OutDir -File | ForEach-Object { Write-Host ("   " + $_.Name + "  " + [math]::Round($_.Length / 1MB, 2) + " MB") }
Write-Host ("   整包: " + $zipOut + "  " + [math]::Round((Get-Item $zipOut).Length / 1MB, 2) + " MB")
