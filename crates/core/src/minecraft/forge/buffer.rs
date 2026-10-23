// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Adaptador opcional de E/S: los JAR oficiales no se reescriben en disco.
use super::*;
use std::io::Write;

const SOURCE: &str = include_str!("BufferAgent.java");

fn argumento_ruta(path: &Path) -> Result<String, Error> {
    Ok(crate::lanzamiento::ruta_java(
        path.to_str().ok_or(Error::Datos)?,
    ))
}

async fn ejecutar_herramienta(
    command: &mut tokio::process::Command,
    cancel: &CancellationToken,
) -> Result<bool, Error> {
    command.stdin(Stdio::null()).kill_on_drop(true);
    for variable in [
        "JAVA_TOOL_OPTIONS",
        "JDK_JAVA_OPTIONS",
        "_JAVA_OPTIONS",
        "CLASSPATH",
    ] {
        command.env_remove(variable);
    }
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let Ok(mut child) = command.spawn() else {
        return Ok(false);
    };
    tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            let _ = child.kill().await;
            Err(Error::Cancelada)
        }
        result = tokio::time::timeout(Duration::from_secs(30), child.wait()) => {
            match result {
                Ok(Ok(status)) => Ok(status.success()),
                _ => { let _ = child.kill().await; Ok(false) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_sin_compilador_conserva_recorrido_original_y_cancelacion() {
        let root = std::env::temp_dir().join(format!("icaro-buffer-{}", random().unwrap()));
        fs::create_dir(&root).unwrap();
        let cancel = CancellationToken::new();
        assert!(preparar(&root, &root.join("java-ausente"), &cancel)
            .await
            .unwrap()
            .is_none());
        assert!(!root.join("buffer-instalador").exists());
        cancel.cancel();
        assert!(matches!(
            preparar(&root, &root.join("java-ausente"), &cancel).await,
            Err(Error::Cancelada)
        ));
        fs::remove_dir(&root).unwrap();
    }
}

pub(super) async fn preparar(
    folder: &Path,
    java: &Path,
    cancel: &CancellationToken,
) -> Result<Option<String>, Error> {
    cancelado(cancel)?;
    let javac = java.with_file_name(if cfg!(windows) { "javac.exe" } else { "javac" });
    if !javac.is_file() {
        return Ok(None);
    }
    let root = folder.join("buffer-instalador");
    segura(folder, &root, true)?;
    fs::create_dir(&root).map_err(|_| Error::Disco)?;
    let source = root.join("BufferAgent.java");
    fs::write(&source, SOURCE).map_err(|_| Error::Disco)?;
    let log = File::create(root.join("preparacion.log")).map_err(|_| Error::Disco)?;
    let mut compile = tokio::process::Command::new(&javac);
    compile
        .args(["-source", "8", "-target", "8", "-encoding", "UTF-8", "-d"])
        .arg(argumento_ruta(&root)?)
        .arg(argumento_ruta(&source)?)
        .stdout(log.try_clone().map_err(|_| Error::Disco)?)
        .stderr(log.try_clone().map_err(|_| Error::Disco)?);
    if !ejecutar_herramienta(&mut compile, cancel).await? {
        return Ok(None);
    }
    let path = root.join("buffer.jar");
    let mut jar = zip::ZipWriter::new(File::create(&path).map_err(|_| Error::Disco)?);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    jar.start_file("META-INF/MANIFEST.MF", options)
        .map_err(|_| Error::Disco)?;
    jar.write_all(b"Manifest-Version: 1.0\r\nPremain-Class: icaro.instalador.BufferAgent\r\n\r\n")
        .map_err(|_| Error::Disco)?;
    for name in [
        "BufferAgent.class",
        "BufferAgent$1.class",
        "BufferAgent$BufferedFile.class",
    ] {
        let entry = format!("icaro/instalador/{name}");
        let bytes = fs::read(root.join(&entry)).map_err(|_| Error::Disco)?;
        if bytes.len() > 256 * 1024 {
            return Ok(None);
        }
        jar.start_file(entry, options).map_err(|_| Error::Disco)?;
        jar.write_all(&bytes).map_err(|_| Error::Disco)?;
    }
    jar.finish().map_err(|_| Error::Disco)?;
    let path = argumento_ruta(&path)?;
    let argument = format!("-javaagent:{path}={path}");
    // Algunos runtimes reducidos no incluyen java.instrument. Se comprueba
    // antes de ejecutar el instalador y se conserva su recorrido original.
    let mut probe = tokio::process::Command::new(java);
    probe
        .arg(&argument)
        .arg("-version")
        .stdout(log.try_clone().map_err(|_| Error::Disco)?)
        .stderr(log);
    if ejecutar_herramienta(&mut probe, cancel).await? {
        Ok(Some(argument))
    } else {
        Ok(None)
    }
}
