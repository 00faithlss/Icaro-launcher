# SPDX-License-Identifier: GPL-3.0-only
Set-StrictMode -Version Latest

function Get-IcaroDecisionPublicacion {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$Canal,
        [Parameter(Mandatory)][string]$Arbol,
        [AllowEmptyCollection()][object[]]$Releases = @(),
        [Parameter(Mandatory)][scriptblock]$ResolverArbol
    )
    if ($Version -notmatch '^\d+\.\d+\.\d+$' -or $Canal -cnotin @('beta','estable') -or $Arbol -notmatch '^[a-f0-9]{40}$') {
        throw [IO.InvalidDataException]::new('Versión, canal o árbol no válidos.')
    }
    $actual = [version]$Version
    $publicadas = @(
        foreach ($release in $Releases) {
            if ($release.draft) { continue }
            $tag = [regex]::Match($release.tag_name,'^v(\d+\.\d+\.\d+)-(beta|build)\.(\d+)$')
            if (-not $tag.Success) { continue }
            $beta = $tag.Groups[2].Value -eq 'beta'
            if ([bool]$release.prerelease -ne $beta) { continue }
            [pscustomobject]@{Release=$release;Version=[version]$tag.Groups[1].Value;Compilacion=[long]$tag.Groups[3].Value;Beta=$beta}
        }
    )
    $previa = $publicadas | Where-Object { $_.Beta -eq ($Canal -eq 'beta') -and $_.Version -eq $actual } |
        Sort-Object Compilacion | Select-Object -Last 1
    if ($previa) {
        $assets = @($previa.Release.assets | ForEach-Object { $_.name })
        $completa = 'latest.json' -in $assets -and "icaro-$Version-windows-x86_64.exe" -in $assets -and "icaro-$Version-windows-x86_64-setup.exe" -in $assets
        if ($completa) {
            # Un fallo de consulta se propaga: no autoriza una publicación.
            $anterior = & $ResolverArbol $previa.Release.tag_name
            if ($anterior -isnot [string] -or $anterior -notmatch '^[a-f0-9]{40}$') {
                throw [IO.InvalidDataException]::new('No se pudo identificar el árbol publicado.')
            }
            if ($anterior -eq $Arbol) { return [pscustomobject]@{Repetida=$true;Motivo="El mismo contenido ya está publicado como $($previa.Release.tag_name)."} }
        }
    }
    $estable = $publicadas | Where-Object { -not $_.Beta } | Sort-Object Version,Compilacion | Select-Object -Last 1
    if ($estable -and $actual -le $estable.Version) {
        throw [IO.InvalidDataException]::new("El canal $Canal tiene $Version y la última estable es $($estable.Version). Sube la versión de Cargo antes de publicar.")
    }
    [pscustomobject]@{Repetida=$false;Motivo='Versión apta para este canal.'}
}

function Test-IcaroArtefactosPublicacion {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Directorio,
        [Parameter(Mandatory)][string]$Revision,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][long]$Compilacion,
        [Parameter(Mandatory)][string]$Canal
    )
    $raiz = Get-Item -LiteralPath $Directorio -ErrorAction Stop
    if (-not $raiz.PSIsContainer -or ($raiz.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw [IO.InvalidDataException]::new('El directorio de artefactos no es una carpeta local.')
    }
    $registros = @(Get-ChildItem -LiteralPath $raiz.FullName -File -Filter '*-artefactos.json')
    if ($registros.Count -ne 1 -or $registros[0].Length -gt 1MB -or ($registros[0].Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw [IO.InvalidDataException]::new('Faltan metadatos únicos y válidos de los artefactos.')
    }
    try { $metadata = Get-Content -LiteralPath $registros[0].FullName -Raw | ConvertFrom-Json -AsHashtable -ErrorAction Stop }
    catch { throw [IO.InvalidDataException]::new('Los metadatos de artefactos no son JSON válido.', $_.Exception) }
    foreach ($campo in @('version','compilacion','canal','revision','prueba','cambios_locales','plataforma','artefactos')) {
        if ($metadata -isnot [Collections.IDictionary] -or -not $metadata.Contains($campo)) {
            throw [IO.InvalidDataException]::new("Falta el campo $campo en los metadatos.")
        }
    }
    if ($Version -notmatch '^\d+\.\d+\.\d+$' -or $Revision -notmatch '^[a-f0-9]{40}$' -or $Canal -cnotin @('beta','estable') -or $Compilacion -le 0 -or
        $metadata.version -cne $Version -or $metadata.revision -cne $Revision -or $metadata.canal -cne $Canal -or
        ($metadata.compilacion -isnot [long] -and $metadata.compilacion -isnot [int]) -or $metadata.compilacion -ne $Compilacion -or
        $metadata.prueba -isnot [bool] -or $metadata.prueba -or $metadata.cambios_locales -isnot [bool] -or $metadata.cambios_locales -or
        $metadata.plataforma -cne 'windows-x86_64') {
        throw [IO.InvalidDataException]::new('Los artefactos no corresponden a esta revisión, versión, canal y compilación de producción.')
    }
    $nombres = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($archivo in @($metadata.artefactos)) {
        if ($archivo -isnot [Collections.IDictionary] -or -not $archivo.Contains('archivo') -or -not $archivo.Contains('bytes') -or -not $archivo.Contains('sha256')) {
            throw [IO.InvalidDataException]::new('Descripción de artefacto incompleta.')
        }
        $nombre = $archivo.archivo
        if ($nombre -isnot [string] -or [string]::IsNullOrWhiteSpace($nombre) -or [IO.Path]::GetFileName($nombre) -cne $nombre -or
            $nombre.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0 -or $nombre -in @('.','..') -or -not $nombres.Add($nombre)) {
            throw [IO.InvalidDataException]::new('Nombre de artefacto fuera de la carpeta o repetido.')
        }
        $ruta = Join-Path $raiz.FullName $nombre
        if (-not (Test-Path -LiteralPath $ruta -PathType Leaf)) { throw [IO.InvalidDataException]::new("Falta el artefacto $nombre.") }
        $f = Get-Item -LiteralPath $ruta
        if (($f.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            ($archivo.bytes -isnot [long] -and $archivo.bytes -isnot [int]) -or $archivo.bytes -le 0 -or $archivo.bytes -ne $f.Length -or
            $archivo.sha256 -isnot [string] -or $archivo.sha256 -notmatch '^[a-f0-9]{64}$' -or
            (Get-FileHash -LiteralPath $ruta -Algorithm SHA256).Hash -ne $archivo.sha256) {
            throw [IO.InvalidDataException]::new("El artefacto $nombre no pasó la comprobación de tamaño e integridad.")
        }
    }
    $portable = "icaro-$Version-windows-x86_64.exe"
    $setup = "icaro-$Version-windows-x86_64-setup.exe"
    if (-not $nombres.Contains($portable) -or -not $nombres.Contains($setup)) {
        throw [IO.InvalidDataException]::new('Faltan el portable o el instalador de esta entrega.')
    }
    [pscustomobject]@{Metadata=$metadata;Portable=(Get-Item -LiteralPath (Join-Path $raiz.FullName $portable));Setup=(Get-Item -LiteralPath (Join-Path $raiz.FullName $setup))}
}

Export-ModuleMember -Function Get-IcaroDecisionPublicacion,Test-IcaroArtefactosPublicacion
