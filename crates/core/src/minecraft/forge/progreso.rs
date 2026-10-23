// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Observa el registro sin bloquear la interfaz ni copiar rutas a sus mensajes.
use super::*;
use tokio::io::AsyncReadExt;

#[derive(Default)]
struct Avance {
    linea: Vec<u8>,
    larga: bool,
    parches: u64,
    bibliotecas: u64,
    fase: &'static str,
}

impl Avance {
    fn recibir(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                if !self.larga {
                    let linea = String::from_utf8_lossy(&self.linea);
                    let linea = linea.trim();
                    if linea.starts_with("Patching ") {
                        self.parches += 1;
                        self.fase = "aplicando parches";
                    } else if linea.starts_with("Considering library ") {
                        self.bibliotecas += 1;
                        self.fase = "preparando bibliotecas";
                    } else if linea == "Building Processor" {
                        self.fase = "procesando el cliente";
                    }
                }
                self.linea.clear();
                self.larga = false;
            } else if self.linea.len() < 1024 && !self.larga {
                self.linea.push(byte);
            } else {
                self.linea.clear();
                self.larga = true;
            }
        }
    }

    fn mensaje(&self, familia: &str, segundos: u64) -> String {
        let detalle = match self.fase {
            "aplicando parches" => format!("aplicando parches ({} clases)", self.parches),
            "preparando bibliotecas" => {
                format!("preparando bibliotecas ({})", self.bibliotecas)
            }
            "" => "iniciando el instalador".into(),
            fase => fase.into(),
        };
        format!("{familia}: {detalle} · {segundos} s")
    }
}

pub(super) async fn esperar(
    child: &mut tokio::process::Child,
    log: &Path,
    familia: &str,
    cancel: &CancellationToken,
    progress: &(impl Fn(Progreso) + Sync),
) -> Result<std::process::ExitStatus, Error> {
    let inicio = tokio::time::Instant::now();
    let limite = inicio + Duration::from_secs(20 * 60);
    let mut reloj = tokio::time::interval(Duration::from_secs(1));
    reloj.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Un fallo al observar el registro no interrumpe el instalador oficial.
    let mut archivo = tokio::fs::File::open(log).await.ok();
    let mut avance = Avance::default();
    let mut buffer = [0; 16 * 1024];
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                let _ = child.kill().await;
                return Err(Error::Cancelada);
            }
            status = child.wait() => return status.map_err(|_| Error::Disco),
            _ = tokio::time::sleep_until(limite) => {
                let _ = child.kill().await;
                return Err(Error::Instalador(familia.into()));
            }
            _ = reloj.tick() => {
                if let Some(file) = archivo.as_mut() {
                    // Trabajo acotado por turno, incluso si el instalador genera
                    // un registro enorme. Se lee desde el último desplazamiento.
                    for _ in 0..16 {
                        match file.read(&mut buffer).await {
                            Ok(0) => break,
                            Ok(n) => avance.recibir(&buffer[..n]),
                            Err(_) => break,
                        }
                    }
                }
                progress(Progreso {
                    etapa: avance.mensaje(familia, inicio.elapsed().as_secs()),
                    // El instalador no declara un total fiable de clases.
                    completados: 0,
                    total: 0,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avance_lee_lineas_fragmentadas_y_no_expone_rutas() {
        let mut avance = Avance::default();
        avance.recibir(b"Considering library org.example:library:1\r\n  Pat");
        assert!(avance.mensaje("forge", 1).contains("bibliotecas (1)"));
        avance.recibir(b"ching ruta/privada/Clase 1/1\n  Patching Otra 1/1\n");
        let mensaje = avance.mensaje("forge", 8);
        assert_eq!(mensaje, "forge: aplicando parches (2 clases) · 8 s");
        assert!(!mensaje.contains("privada"));
    }

    #[test]
    fn registro_sin_salto_no_crece_sin_limite_y_recupera_la_siguiente_linea() {
        let mut avance = Avance::default();
        avance.recibir(&vec![b'x'; 512 * 1024]);
        assert!(avance.linea.len() <= 1024);
        avance.recibir(b"\nBuilding Processor\n");
        assert_eq!(
            avance.mensaje("neoforge", 4),
            "neoforge: procesando el cliente · 4 s"
        );
    }

    #[tokio::test]
    async fn cancelar_mientras_espera_termina_el_proceso() {
        #[cfg(windows)]
        let mut comando = {
            let mut command = tokio::process::Command::new("powershell.exe");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ]);
            command.creation_flags(0x08000000);
            command
        };
        #[cfg(not(windows))]
        let mut comando = {
            let mut command = tokio::process::Command::new("sleep");
            command.arg("60");
            command
        };
        let mut child = comando.kill_on_drop(true).spawn().unwrap();
        let cancel = CancellationToken::new();
        let log = std::env::temp_dir().join(format!("icaro-progreso-{}.log", std::process::id()));
        fs::write(&log, b"  Patching ejemplo/Clase 1/1\n").unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            esperar(&mut child, &log, "forge", &cancel, &|p| {
                // Confirma que se publica el avance antes de cancelar,
                // mientras el proceso sigue esperando.
                if p.etapa.contains("(1 clases)") {
                    cancel.cancel();
                }
            }),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(Error::Cancelada)));
        assert!(child.try_wait().unwrap().is_some());
        fs::remove_file(log).unwrap();
    }
}
