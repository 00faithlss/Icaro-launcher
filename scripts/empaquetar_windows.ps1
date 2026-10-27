# SPDX-License-Identifier: GPL-3.0-only
[CmdletBinding()]
param(
    [string]$Compilador = 'ISCC.exe',
    [string]$Binario,
    [string]$Servicio,
    [string]$Salida,
    [ValidateRange(1,16)][int]$Trabajos = 1,
    [switch]$Medir,
    [switch]$Prueba
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'El empaquetado requiere Windows y PowerShell 7.' }
$taskRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$taskChannel = if ($env:ICARO_CANAL) { $env:ICARO_CANAL } else { 'estable' }
if ($taskChannel -notin @('beta','estable')) { throw 'Canal de compilación inválido.' }
if (-not $Salida) { $Salida = Join-Path $taskRoot 'target\dist\windows' }
$taskOutput = [IO.Path]::GetFullPath($Salida)
New-Item -ItemType Directory -Force -Path $taskOutput | Out-Null
$taskCompiler = (Get-Command $Compilador -ErrorAction Stop).Source
Push-Location $taskRoot
try {
    $taskMetadata = (& cargo metadata --locked --format-version 1 --no-deps | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0) { throw 'No se pudo leer la versión del workspace.' }
    $taskVersion = ($taskMetadata.packages | Where-Object name -EQ 'icaro-ui').version
    if ($taskVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'El instalador requiere una versión numérica completa.' }
    if (-not $Binario) {
        if ($Prueba) { throw 'Para empaquetar una compilación de prueba indica -Binario y -Servicio.' }
        $taskBuildDir = Join-Path $taskRoot 'target\windows-package-build'
        $taskBuildArgs = @('build','--locked','--release','-j',"$Trabajos",'--target-dir',$taskBuildDir,'--target','x86_64-pc-windows-msvc','--config','target.x86_64-pc-windows-msvc.rustflags=["-C","target-feature=+crt-static"]')
        if ($Medir) { $taskBuildArgs += '--timings' }
        $Binario = Join-Path $taskBuildDir 'x86_64-pc-windows-msvc\release\examples\launcher.exe'
        $Servicio = Join-Path $taskBuildDir 'x86_64-pc-windows-msvc\release\examples\sync_host.exe'
        # Una sola invocación unifica las features: icaro-core y sus dependencias
        # se compilan una vez para el launcher y el servicio.
        $taskCode = $null
        $taskTimer = [Diagnostics.Stopwatch]::StartNew()
        try {
            & cargo @taskBuildArgs -p icaro-ui -p icaro-core --example launcher --example sync_host
            $taskCode = $LASTEXITCODE
        } finally {
            $taskTimer.Stop()
            if ($Medir) {
                $taskDiagnostics = Join-Path $taskRoot 'target/ci'
                New-Item -ItemType Directory -Force -Path $taskDiagnostics | Out-Null
                $taskBytes = [ordered]@{}
                foreach ($taskExe in @($Binario,$Servicio)) {
                    $taskBytes[[IO.Path]::GetFileNameWithoutExtension($taskExe)] = if ($taskCode -eq 0 -and (Test-Path -LiteralPath $taskExe)) { (Get-Item -LiteralPath $taskExe).Length } else { $null }
                }
                ConvertTo-Json -InputObject @([ordered]@{ejemplo='launcher+sync_host';segundos=$taskTimer.Elapsed.TotalSeconds;salida=$taskCode;trabajos=$Trabajos;bytes=$taskBytes}) -Depth 3 |
                    Set-Content -LiteralPath (Join-Path $taskDiagnostics 'release-tiempos.json') -Encoding utf8
            }
        }
        if ($taskCode -ne 0) { throw 'Falló la compilación del launcher o del servicio de mods.' }
    }
    if (-not $Servicio) { throw 'Indica el ejecutable del servicio de mods.' }
    $taskBinary = (Resolve-Path -LiteralPath $Binario).Path
    $taskService = (Resolve-Path -LiteralPath $Servicio).Path
    # Cabecera e importaciones reales del PE: x86-64 y CRT integrado en producción.
    function Read-PeImports([string]$Path) {
        $taskBytes = [IO.File]::ReadAllBytes($Path)
        if ($taskBytes.Length -lt 512) { throw 'Ejecutable inválido.' }
        $taskPe = [BitConverter]::ToInt32($taskBytes, 60)
        if ($taskPe -lt 64 -or $taskPe + 256 -ge $taskBytes.Length -or [BitConverter]::ToUInt32($taskBytes,$taskPe) -ne 0x4550 -or [BitConverter]::ToUInt16($taskBytes,$taskPe+4) -ne 0x8664) { throw 'Se requiere un PE Windows x86-64.' }
        $taskOptional = $taskPe + 24
        if ([BitConverter]::ToUInt16($taskBytes,$taskOptional) -ne 0x20b) { throw 'Cabecera PE32+ inválida.' }
        $taskSections = $taskOptional + [BitConverter]::ToUInt16($taskBytes,$taskPe+20)
        $taskCount = [BitConverter]::ToUInt16($taskBytes,$taskPe+6)
        function Convert-Rva([uint32]$Rva) {
            for ($taskI=0; $taskI -lt $taskCount; $taskI++) {
                $taskSection = $taskSections + $taskI*40
                $taskVa = [BitConverter]::ToUInt32($taskBytes,$taskSection+12)
                $taskSize = [Math]::Max([BitConverter]::ToUInt32($taskBytes,$taskSection+8),[BitConverter]::ToUInt32($taskBytes,$taskSection+16))
                if ($Rva -ge $taskVa -and $Rva -lt $taskVa+$taskSize) { return [int]($Rva-$taskVa+[BitConverter]::ToUInt32($taskBytes,$taskSection+20)) }
            }
            throw 'Tabla de importaciones fuera del PE.'
        }
        $taskRva = [BitConverter]::ToUInt32($taskBytes,$taskOptional+120)
        if ($taskRva -eq 0) { return @() }
        $taskTable = Convert-Rva $taskRva
        $taskNames = @()
        for ($taskI=0; $taskI -lt 512; $taskI++) {
            $taskNameRva = [BitConverter]::ToUInt32($taskBytes,$taskTable+$taskI*20+12)
            if ($taskNameRva -eq 0) { return $taskNames }
            $taskOffset = Convert-Rva $taskNameRva
            $taskEnd = $taskOffset
            while ($taskEnd -lt $taskBytes.Length -and $taskEnd-$taskOffset -lt 260 -and $taskBytes[$taskEnd] -ne 0) { $taskEnd++ }
            if ($taskEnd-$taskOffset -ge 260) { throw 'Nombre de importación inválido.' }
            $taskNames += [Text.Encoding]::ASCII.GetString($taskBytes,$taskOffset,$taskEnd-$taskOffset)
        }
        throw 'Demasiadas importaciones PE.'
    }
    $taskImports = @(Read-PeImports $taskBinary) + @(Read-PeImports $taskService)
    if (-not $Prueba -and ($taskImports | Where-Object { $_ -match '^(vcruntime|msvcp)\d+.*\.dll$' })) { throw 'La compilación depende de Visual C++ externo. Usa el CRT estático del script.' }
    if ((Get-Item -LiteralPath $taskBinary).VersionInfo.ProductVersion -ne $taskVersion) { throw 'La versión del ejecutable no coincide con Cargo.' }
    $taskRevision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'No se pudo identificar la revisión de Git.' }
    $taskDirty = [bool](& git status --porcelain)
    $taskSuffix = if ($Prueba) { '-prueba' } else { '' }
    $taskBase = "icaro-$taskVersion-windows-x86_64$taskSuffix"
    $taskStage = Join-Path $taskOutput ('.preparacion-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path (Join-Path $taskStage 'licencias') -Force | Out-Null
    Copy-Item -LiteralPath $taskBinary -Destination (Join-Path $taskStage 'IcaroLauncher.exe')
    Copy-Item -LiteralPath $taskService -Destination (Join-Path $taskStage 'icaro-sync-host.exe')
    Copy-Item -LiteralPath (Join-Path $taskRoot 'LICENSE'),(Join-Path $taskRoot 'docs\creditos.md'),(Join-Path $taskRoot 'crates\ui\assets\icono\FUENTES-Y-LICENCIAS.md'),(Join-Path $taskRoot 'crates\ui\assets\iconos\LICENSE-pixelarticons.txt'),(Join-Path $taskRoot 'crates\ui\assets\fonts\OFL-Archivo.txt'),(Join-Path $taskRoot 'crates\ui\assets\fonts\OFL-MartianMono.txt') -Destination (Join-Path $taskStage 'licencias')
    $taskDeps = & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'No se pudieron reunir los avisos de dependencias.' }
    $taskLicenseIndex = foreach ($taskPackage in $taskDeps.packages | Where-Object { $_.source }) {
        $taskPackageRoot = Split-Path -Parent $taskPackage.manifest_path
        $taskNotices = @(Get-ChildItem -LiteralPath $taskPackageRoot -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|COPYRIGHT|NOTICE)' })
        $taskNoticeDir = Join-Path $taskStage ('licencias\dependencias\' + $taskPackage.name + '-' + $taskPackage.version)
        if ($taskNotices.Count) {
            New-Item -ItemType Directory -Force -Path $taskNoticeDir | Out-Null
            foreach ($taskNotice in $taskNotices) { Copy-Item -LiteralPath $taskNotice.FullName -Destination $taskNoticeDir }
        }
        "$($taskPackage.name) $($taskPackage.version) | $($taskPackage.license) | $($taskPackage.repository) | avisos: $($taskNotices.Count)"
    }
    $taskLicenseIndex | Set-Content -LiteralPath (Join-Path $taskStage 'licencias\dependencias.txt') -Encoding utf8
    try {
        & $taskCompiler "/DVersion=$taskVersion" "/DRaiz=$taskRoot" "/DPreparacion=$taskStage" "/DSalida=$taskOutput" "/DNombre=$taskBase-setup" (Join-Path $taskRoot 'packaging\windows\icaro.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Falló la creación del instalador.' }
        $taskExe = Join-Path $taskOutput "$taskBase.exe"
        Copy-Item -LiteralPath $taskBinary -Destination $taskExe
        $taskZip = Join-Path $taskOutput "$taskBase-portable.zip"
        if (Test-Path -LiteralPath $taskZip) { Remove-Item -LiteralPath $taskZip }
        Compress-Archive -Path (Join-Path $taskStage '*') -DestinationPath $taskZip
        $taskArtifacts = @($taskExe,$taskZip,(Join-Path $taskOutput "$taskBase-setup.exe")) | ForEach-Object {
            [pscustomobject][ordered]@{ archivo=[IO.Path]::GetFileName($_); bytes=(Get-Item -LiteralPath $_).Length; sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant() }
        }
        $taskBuildNumber = if ($env:ICARO_COMPILACION) { [long]$env:ICARO_COMPILACION } else { 0 }
        [ordered]@{version=$taskVersion;compilacion=$taskBuildNumber;canal=$taskChannel;revision=$taskRevision;cambios_locales=$taskDirty;prueba=[bool]$Prueba;plataforma='windows-x86_64';firmado=$false;importaciones=@($taskImports | Sort-Object -Unique);artefactos=$taskArtifacts} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $taskOutput "$taskBase-artefactos.json") -Encoding utf8
        $taskArtifacts | Format-Table archivo,bytes,sha256
    } finally {
        # Solo se elimina el directorio aleatorio creado por esta invocación.
        $taskResolvedStage = [IO.Path]::GetFullPath($taskStage)
        if (-not $taskResolvedStage.StartsWith($taskOutput + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Preparación fuera de la carpeta de salida.' }
        Remove-Item -LiteralPath $taskResolvedStage -Recurse -Force
    }
} finally { Pop-Location }
