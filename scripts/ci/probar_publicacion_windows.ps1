# SPDX-License-Identifier: GPL-3.0-only
[CmdletBinding()]
param([ValidateSet('Empaquetado','Publicacion','Todo')][string]$Grupo = 'Todo')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$script:aprobadas = 0
$script:fallidas = 0
function Comprobar([bool]$Condicion, [string]$Mensaje) {
    if (-not $Condicion) { throw $Mensaje }
}
function Probar([string]$Nombre, [scriptblock]$Accion) {
    try { & $Accion; $script:aprobadas++; Write-Host "OK $Nombre" }
    catch { $script:fallidas++; Write-Host "FALLO ${Nombre}: $($_.Exception.Message)" }
}
if ($Grupo -in @('Empaquetado','Todo')) {
    $taskTemp = Join-Path ([IO.Path]::GetTempPath()) ('icaro-ci-pruebas-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $taskTemp | Out-Null
    # `Function:global:cargo` no es una ruta válida del proveedor y Remove-Item
    # la ignoraba sin error, dejando el sustituto activo para el Cargo real.
    # Se guarda el bloque, no la función: redefinirla modifica ese mismo objeto.
    $taskCargoAnterior = (Get-Item -LiteralPath Function:\cargo -ErrorAction SilentlyContinue)?.ScriptBlock
    $global:IcaroTestCargoCalls = [Collections.Generic.List[object]]::new()
    $global:IcaroTestFallarBuild = 2
    # Solo se sustituye la frontera externa de Cargo; se ejecuta el script real.
    function global:cargo {
        if ($args[0] -eq 'metadata') {
            $global:LASTEXITCODE = 0
            '{"packages":[{"name":"icaro-ui","version":"0.3.3"}]}'
            return
        }
        $global:IcaroTestCargoCalls.Add(@($args))
        $global:LASTEXITCODE = if ($global:IcaroTestCargoCalls.Count -eq $global:IcaroTestFallarBuild) { 1 } else { 0 }
    }
    try {
        Probar 'una sola compilación incluye launcher y servicio con el paralelismo, CRT y target' {
            $global:IcaroTestCargoCalls.Clear()
            # El fallo detiene el script antes de empaquetar; basta para inspeccionar la invocación.
            $global:IcaroTestFallarBuild = 1
            $errorBuild = ''
            try { & (Join-Path $taskRoot 'scripts/empaquetar_windows.ps1') -Compilador (Join-Path $PSHOME 'pwsh.exe') -Salida (Join-Path $taskTemp 'salida') -Trabajos 2 -Medir }
            catch { $errorBuild = $_.Exception.Message }
            Comprobar ($errorBuild -eq 'Falló la compilación del launcher o del servicio de mods.') "Error inesperado: $errorBuild"
            Comprobar ($global:IcaroTestCargoCalls.Count -eq 1) 'Launcher y servicio deben compilarse en una invocación.'
            $invocacion = $global:IcaroTestCargoCalls[0]
            foreach ($par in @(@('-p','icaro-ui'),@('-p','icaro-core'),@('--example','launcher'),@('--example','sync_host'))) {
                $hallado = $false
                for ($i = 0; $i -lt $invocacion.Count - 1; $i++) { if ($invocacion[$i] -ceq $par[0] -and $invocacion[$i + 1] -ceq $par[1]) { $hallado = $true } }
                Comprobar $hallado "Falta $($par -join ' ')."
            }
            foreach ($invocacion in $global:IcaroTestCargoCalls) {
                $n = [Array]::IndexOf($invocacion, '-j')
                Comprobar ($n -ge 0 -and $invocacion[$n + 1] -eq '2') 'No se propagó el paralelismo.'
                Comprobar ($invocacion -contains '--timings') 'Falta el informe de Cargo.'
                Comprobar ($invocacion -contains 'x86_64-pc-windows-msvc') 'Se perdió el target de producción.'
                Comprobar ($invocacion -contains 'target.x86_64-pc-windows-msvc.rustflags=["-C","target-feature=+crt-static"]') 'Se perdió CRT estático.'
                $n = [Array]::IndexOf($invocacion, '--target-dir')
                Comprobar ($n -ge 0 -and $invocacion[$n + 1] -eq (Join-Path $taskRoot 'target/windows-package-build')) 'Cambió la carpeta de producción.'
            }
        }
        foreach ($invalido in @(0,17)) {
            Probar "rechaza $invalido antes de invocar Cargo" {
                $global:IcaroTestCargoCalls.Clear()
                $rechazado = $false
                try { & (Join-Path $taskRoot 'scripts/empaquetar_windows.ps1') -Compilador (Join-Path $PSHOME 'pwsh.exe') -Salida (Join-Path $taskTemp 'invalida') -Trabajos $invalido }
                catch { $rechazado = $_.FullyQualifiedErrorId -like 'ParameterArgumentValidationError*' }
                Comprobar $rechazado 'El número de trabajos no se validó en el enlace de parámetros.'
                Comprobar ($global:IcaroTestCargoCalls.Count -eq 0) 'Cargo no debe ejecutarse con un valor inválido.'
            }
        }
        Probar 'un fallo de compilación impide empaquetar' {
            $global:IcaroTestCargoCalls.Clear()
            $global:IcaroTestFallarBuild = 1
            $errorBuild = ''
            try { & (Join-Path $taskRoot 'scripts/empaquetar_windows.ps1') -Compilador (Join-Path $PSHOME 'pwsh.exe') -Salida (Join-Path $taskTemp 'fallo') -Trabajos 1 }
            catch { $errorBuild = $_.Exception.Message }
            Comprobar ($errorBuild -eq 'Falló la compilación del launcher o del servicio de mods.') "No propagó el error de compilación: $errorBuild"
            Comprobar ($global:IcaroTestCargoCalls.Count -eq 1) 'No debe continuar después de un error.'
            Comprobar (@(Get-ChildItem -LiteralPath (Join-Path $taskTemp 'fallo') -File).Count -eq 0) 'No deben publicarse paquetes tras el error.'
        }
    } finally {
        if ($taskCargoAnterior) { Set-Item -LiteralPath Function:\cargo -Value $taskCargoAnterior }
        else { Remove-Item -LiteralPath Function:\cargo }
        Remove-Variable IcaroTestCargoCalls,IcaroTestFallarBuild -Scope Global
        # La carpeta se creó exclusivamente para esta ejecución; no contiene datos del usuario.
        $taskResolved = [IO.Path]::GetFullPath($taskTemp)
        $taskTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar
        if (-not $taskResolved.StartsWith($taskTempRoot,[StringComparison]::OrdinalIgnoreCase) -or -not ([IO.Path]::GetFileName($taskResolved)).StartsWith('icaro-ci-pruebas-')) { throw 'Carpeta de pruebas fuera del temporal.' }
        Remove-Item -LiteralPath $taskResolved -Recurse -Force
    }
    Probar 'restaura el Cargo real tras las pruebas de empaquetado' {
        $taskActual = Get-Item -LiteralPath Function:\cargo -ErrorAction SilentlyContinue
        if ($taskCargoAnterior) { Comprobar ($taskActual -and "$($taskActual.ScriptBlock)" -ceq "$taskCargoAnterior") 'No se restauró la función cargo previa.' }
        else { Comprobar (-not $taskActual) 'El sustituto de Cargo sigue activo.' }
    }
}
if ($Grupo -in @('Publicacion','Todo')) {
    $taskModule = Join-Path $PSScriptRoot 'publicacion_windows.psm1'
    if (Test-Path -LiteralPath $taskModule) { Import-Module $taskModule -Force }
    function DebeFallar([scriptblock]$Accion, [type]$Tipo = [IO.InvalidDataException]) {
        $fallo = $false
        try { & $Accion | Out-Null }
        catch { Comprobar ($_.Exception -is $Tipo) "Error inesperado: $($_.Exception.Message)"; $fallo = $true }
        Comprobar $fallo 'La operación debía rechazar estos datos.'
    }
    function ReleasePrueba([string]$Tag, [bool]$Beta = $false) {
        [pscustomobject]@{ tag_name=$Tag;prerelease=$Beta;draft=$false;assets=@(
            [pscustomobject]@{name='latest.json'},
            [pscustomobject]@{name='icaro-0.3.3-windows-x86_64.exe'},
            [pscustomobject]@{name='icaro-0.3.3-windows-x86_64-setup.exe'}
        ) }
    }
    Probar 'omite una entrega idéntica ya publicada en su canal' {
        $r = Get-IcaroDecisionPublicacion -Version '0.3.3' -Canal estable -Arbol ('a'*40) -Releases @(ReleasePrueba 'v0.3.3-build.42') -ResolverArbol { param($tag) 'a'*40 }
        Comprobar $r.Repetida 'Debe reconocer la entrega idéntica.'
    }
    Probar 'la beta idéntica permite publicar su primera estable' {
        $r = Get-IcaroDecisionPublicacion -Version '0.3.3' -Canal estable -Arbol ('a'*40) -Releases @(ReleasePrueba 'v0.3.3-beta.42' $true) -ResolverArbol { throw 'No corresponde consultar otra beta.' }
        Comprobar (-not $r.Repetida) 'La beta no sustituye la publicación estable.'
    }
    Probar 'otra revisión de la misma versión estable se rechaza' {
        DebeFallar { Get-IcaroDecisionPublicacion -Version '0.3.3' -Canal estable -Arbol ('b'*40) -Releases @(ReleasePrueba 'v0.3.3-build.42') -ResolverArbol { 'a'*40 } }
    }
    Probar 'beta de versión igual a la estable se rechaza' {
        DebeFallar { Get-IcaroDecisionPublicacion -Version '0.3.3' -Canal beta -Arbol ('b'*40) -Releases @(ReleasePrueba 'v0.3.3-build.42') -ResolverArbol { 'a'*40 } }
    }
    Probar 'la revisión que quedó obsoleta durante el build se rechaza al revalidar' {
        DebeFallar { Get-IcaroDecisionPublicacion -Version '0.3.2' -Canal estable -Arbol ('b'*40) -Releases @(ReleasePrueba 'v0.3.3-build.42') -ResolverArbol { 'a'*40 } }
    }
    Probar 'primera publicación sin releases anteriores es válida' {
        $r = Get-IcaroDecisionPublicacion -Version '0.3.3' -Canal estable -Arbol ('a'*40) -Releases @() -ResolverArbol { throw 'No hay tag.' }
        Comprobar (-not $r.Repetida) 'Debe permitir una primera entrega.'
    }
    Probar 'un error consultando el árbol no se convierte en permiso para publicar' {
        DebeFallar { Get-IcaroDecisionPublicacion -Version '0.3.3' -Canal estable -Arbol ('a'*40) -Releases @(ReleasePrueba 'v0.3.3-build.42') -ResolverArbol { throw [InvalidOperationException]::new('Sin red') } } ([InvalidOperationException])
    }
    $taskPubTemp = Join-Path ([IO.Path]::GetTempPath()) ('icaro-ci-pruebas-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $taskPubTemp | Out-Null
    function NuevaEntregaPrueba {
        $dir = Join-Path $taskPubTemp ([guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $dir | Out-Null
        $items = @()
        foreach ($nombre in @('icaro-0.3.3-windows-x86_64.exe','icaro-0.3.3-windows-x86_64-setup.exe','icaro-0.3.3-windows-x86_64-portable.zip')) {
            [IO.File]::WriteAllBytes((Join-Path $dir $nombre),[byte[]]@(97,98,99))
            $items += [ordered]@{archivo=$nombre;bytes=3;sha256='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'}
        }
        $meta = [ordered]@{version='0.3.3';compilacion=42;canal='beta';revision=('a'*40);prueba=$false;cambios_locales=$false;plataforma='windows-x86_64';artefactos=$items}
        $ruta = Join-Path $dir 'icaro-0.3.3-windows-x86_64-artefactos.json'
        $meta | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ruta -Encoding utf8
        @{Directorio=$dir;Ruta=$ruta;Metadata=$meta}
    }
    function ValidarEntregaPrueba($Fixture) {
        Test-IcaroArtefactosPublicacion -Directorio $Fixture.Directorio -Revision ('a'*40) -Version '0.3.3' -Compilacion 42 -Canal beta
    }
    try {
        Probar 'artefactos verificados conservan portable y setup de la entrega' {
            $f = NuevaEntregaPrueba
            $r = ValidarEntregaPrueba $f
            Comprobar ($r.Portable.Name -eq 'icaro-0.3.3-windows-x86_64.exe') 'Portable incorrecto.'
            Comprobar ($r.Setup.Name -eq 'icaro-0.3.3-windows-x86_64-setup.exe') 'Setup incorrecto.'
        }
        foreach ($caso in @(
            @{Nombre='otra revisión';Campo='revision';Valor=('b'*40)},
            @{Nombre='otro canal';Campo='canal';Valor='estable'},
            @{Nombre='otra compilación';Campo='compilacion';Valor=43},
            @{Nombre='otra versión';Campo='version';Valor='0.3.2'},
            @{Nombre='compilación fraccionaria';Campo='compilacion';Valor=42.5},
            @{Nombre='paquete de prueba';Campo='prueba';Valor=$true},
            @{Nombre='fuentes con cambios locales';Campo='cambios_locales';Valor=$true}
        )) {
            Probar "rechaza $($caso.Nombre)" {
                $f = NuevaEntregaPrueba
                $f.Metadata[$caso.Campo] = $caso.Valor
                $f.Metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $f.Ruta -Encoding utf8
                DebeFallar { ValidarEntregaPrueba $f }
            }
        }
        Probar 'rechaza un archivo alterado aunque conserve su tamaño' {
            $f = NuevaEntregaPrueba
            [IO.File]::WriteAllBytes((Join-Path $f.Directorio 'icaro-0.3.3-windows-x86_64.exe'),[byte[]]@(97,98,100))
            DebeFallar { ValidarEntregaPrueba $f }
        }
        Probar 'rechaza la ausencia del setup' {
            $f = NuevaEntregaPrueba
            Remove-Item -LiteralPath (Join-Path $f.Directorio 'icaro-0.3.3-windows-x86_64-setup.exe')
            DebeFallar { ValidarEntregaPrueba $f }
        }
        Probar 'rechaza nombres de artefacto que escapan de su carpeta' {
            $f = NuevaEntregaPrueba
            $f.Metadata.artefactos[0].archivo = '../ajeno.exe'
            $f.Metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $f.Ruta -Encoding utf8
            DebeFallar { ValidarEntregaPrueba $f }
        }
        Probar 'rechaza metadatos incompletos' {
            $f = NuevaEntregaPrueba
            $f.Metadata.Remove('canal')
            $f.Metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $f.Ruta -Encoding utf8
            DebeFallar { ValidarEntregaPrueba $f }
        }
    } finally {
        $taskResolved = [IO.Path]::GetFullPath($taskPubTemp)
        $taskTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar
        if (-not $taskResolved.StartsWith($taskTempRoot,[StringComparison]::OrdinalIgnoreCase) -or -not ([IO.Path]::GetFileName($taskResolved)).StartsWith('icaro-ci-pruebas-')) { throw 'Carpeta de pruebas fuera del temporal.' }
        Remove-Item -LiteralPath $taskResolved -Recurse -Force
    }
}
if ($Grupo -in @('Publicacion','Todo')) {
    # Comprobación textual de las puertas; no sustituye a actionlint ni a una ejecución remota.
    $taskWorkflow = Get-Content -LiteralPath (Join-Path $taskRoot '.github/workflows/windows-package.yml') -Raw
    $taskJobs = [regex]::Split($taskWorkflow, '(?m)^  (?=publicar:)')
    Probar 'solo un push con construcción correcta y no repetida habilita publicar' {
        Comprobar ($taskJobs.Count -eq 2) 'Falta el job publicar.'
        Comprobar ($taskJobs[1] -match "(?m)^    if: github\.event_name == 'push' && needs\.windows\.result == 'success' && needs\.windows\.outputs\.repetida != 'true'\r?$") 'La condición de publicar cambió.'
        Comprobar ($taskJobs[1] -match '(?m)^    needs: windows\r?$') 'publicar no depende de windows.'
    }
    Probar 'el bloqueo global y la escritura quedan solo en publicar' {
        Comprobar ($taskJobs[0] -notmatch 'icaro-windows-publication' -and $taskJobs[1] -match 'group: icaro-windows-publication') 'El bloqueo global no está solo en publicar.'
        Comprobar ($taskJobs[0] -notmatch 'contents: write' -and $taskJobs[1] -match 'contents: write') 'El permiso de escritura no está solo en publicar.'
        Comprobar ($taskJobs[0] -notmatch 'gh release create') 'La construcción no debe publicar.'
        Comprobar ($taskWorkflow -notmatch 'cancel-in-progress: true') 'Una publicación activa no debe cancelarse.'
    }
    Probar 'el ensayo de perfil release se limita a ejecuciones manuales' {
        Comprobar ($taskJobs[0] -match "ICARO_PERFIL_RELEASE: \$\{\{ inputs\.perfil_release \|\| 'distribucion' \}\}") 'Un push debe usar el perfil de distribución.'
        Comprobar ($taskJobs[0] -match "-ne 'workflow_dispatch'\) \{ throw 'El ensayo de perfil solo se admite") 'Falta el rechazo del ensayo fuera de ejecuciones manuales.'
        Comprobar ($taskJobs[1] -notmatch 'CARGO_PROFILE_RELEASE') 'publicar no debe alterar el perfil release.'
    }
    Probar 'la clave de caché sigue al LTO del perfil release' {
        $taskCargo = Get-Content -LiteralPath (Join-Path $taskRoot 'Cargo.toml') -Raw
        $taskRelease = [regex]::Match($taskCargo, '(?ms)^\[profile\.release\]\s*$(.*?)(?=^\[|\z)')
        Comprobar $taskRelease.Success 'Falta el perfil release.'
        $taskLto = [regex]::Match($taskRelease.Groups[1].Value, '(?m)^lto\s*=\s*"?([a-z]+)"?')
        Comprobar $taskLto.Success 'El perfil release no declara lto.'
        # rust-cache ignora `key` cuando hay `shared-key`: el perfil va en esta última.
        $taskRustCache = [regex]::Match($taskJobs[0], '(?s)uses: Swatinem/rust-cache@.*?(?=\r?\n      - )').Value
        Comprobar ($taskRustCache -ne '') 'Falta el paso de rust-cache.'
        Comprobar ($taskRustCache -notmatch '(?m)^\s+key:') 'rust-cache ignora `key` junto a `shared-key`.'
        $taskClave = [regex]::Match($taskRustCache,'(?m)^\s+shared-key: icaro-windows-perfil-\$\{\{ env\.ICARO_PERFIL_RELEASE \}\}-lto-([a-z]+)\r?$')
        Comprobar $taskClave.Success 'La caché de Cargo no incluye el perfil en su clave.'
        Comprobar ($taskClave.Groups[1].Value -ceq $taskLto.Groups[1].Value) "La clave usa lto-$($taskClave.Groups[1].Value) y Cargo.toml lto=$($taskLto.Groups[1].Value)."
    }
    Probar 'publicar revalida la versión y los artefactos de esta ejecución' {
        $taskOrden = @('Get-IcaroDecisionPublicacion','gh run download \$env:GITHUB_RUN_ID','Test-IcaroArtefactosPublicacion','gh release create') |
            ForEach-Object { $m = [regex]::Match($taskJobs[1], $_); Comprobar $m.Success "Falta $_."; $m.Index }
        Comprobar ((($taskOrden | Sort-Object) -join ',') -eq ($taskOrden -join ',')) 'El orden de comprobaciones de publicar cambió.'
    }
}
Write-Host "Pruebas: $script:aprobadas aprobadas; $script:fallidas fallidas."
if ($script:fallidas -gt 0) { exit 1 }
exit 0
