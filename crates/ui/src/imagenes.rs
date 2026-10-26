// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Decodificación fuera del hilo de interfaz y caché acotada de píxeles.
//!
//! iced puede descartar sus texturas al cambiar de pantalla. Entregar RGBA
//! permite volver a subirlas sin decodificar imágenes durante layout/draw.
//! Los recortes y las vistas previas se preparan al compilar.

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex, OnceLock};

use iced::widget::image::Handle;
use image::RgbaImage;

use crate::laminas::Lamina;
use crate::tema::Modo;

const PRESUPUESTO: usize = 128 * 1024 * 1024;
#[cfg(test)]
const LADO_MAX: u32 = 2048;
type Clave = (Lamina, Modo);

enum Entrada {
    Pendiente {
        previa: Handle,
    },
    Lista {
        handle: Handle,
        bytes: usize,
        uso: u64,
    },
    Fallida {
        previa: Handle,
    },
}

#[derive(Default)]
struct Estado {
    entradas: HashMap<Clave, Entrada>,
    bytes: usize,
    reloj: u64,
}

impl Estado {
    fn insertar(&mut self, clave: Clave, handle: Handle, bytes: usize, limite: usize) {
        while self.bytes + bytes > limite {
            let antigua = self
                .entradas
                .iter()
                .filter_map(|(k, e)| match e {
                    Entrada::Lista { uso, .. } => Some((*k, *uso)),
                    _ => None,
                })
                .min_by_key(|(_, uso)| *uso)
                .map(|(k, _)| k);
            let Some(antigua) = antigua else { break };
            if let Some(Entrada::Lista { bytes, .. }) = self.entradas.remove(&antigua) {
                self.bytes -= bytes;
            }
        }
        self.reloj += 1;
        self.bytes += bytes;
        self.entradas.insert(
            clave,
            Entrada::Lista {
                handle,
                bytes,
                uso: self.reloj,
            },
        );
    }
}

struct Cache {
    estado: Mutex<Estado>,
    pedidos: mpsc::Sender<Clave>,
    cambios: tokio::sync::watch::Sender<u64>,
}

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| {
        let (pedidos, recibidos) = mpsc::channel::<Clave>();
        let recibidos = Arc::new(Mutex::new(recibidos));
        let (cambios, _) = tokio::sync::watch::channel(0);
        // El hilo consulta CACHE solamente después de recibir el primer pedido,
        // cuando OnceLock ya terminó de inicializarse.
        for indice in 0..2 {
            let recibidos = Arc::clone(&recibidos);
            std::thread::Builder::new()
                .name(format!("icaro-imagenes-{indice}"))
                .spawn(move || loop {
                    let clave = {
                        let recibidos = recibidos.lock().unwrap();
                        let Ok(clave) = recibidos.recv() else { break };
                        clave
                    };
                    let resultado = preparar(clave.0, clave.1);
                    let cache = cache();
                    let mut estado = cache.estado.lock().unwrap();
                    match resultado {
                        Ok(imagen) => {
                            let (w, h) = imagen.dimensions();
                            let pixels = imagen.into_raw();
                            let bytes = pixels.len();
                            estado.insertar(
                                clave,
                                Handle::from_rgba(w, h, pixels),
                                bytes,
                                PRESUPUESTO,
                            );
                        }
                        Err(error) => {
                            eprintln!("No se pudo cargar {:?}: {error}", clave.0);
                            let previa = clave.0.previa(clave.1);
                            estado.entradas.insert(clave, Entrada::Fallida { previa });
                        }
                    }
                    drop(estado);
                    cache.cambios.send_modify(|generacion| *generacion += 1);
                })
                .expect("iniciar decodificación de imágenes");
        }
        Cache {
            estado: Mutex::new(Estado::default()),
            pedidos,
            cambios,
        }
    })
}

pub(crate) fn imagen(lamina: Lamina, modo: Modo) -> Handle {
    let cache = cache();
    let modo = lamina.modo_propio(modo);
    let clave = (lamina, modo);
    let mut estado = cache.estado.lock().unwrap();
    estado.reloj += 1;
    let reloj = estado.reloj;
    match estado.entradas.get_mut(&clave) {
        Some(Entrada::Lista { handle, uso, .. }) => {
            *uso = reloj;
            return handle.clone();
        }
        Some(Entrada::Pendiente { previa } | Entrada::Fallida { previa }) => return previa.clone(),
        None => {
            let previa = lamina.previa(modo);
            estado.entradas.insert(
                clave,
                Entrada::Pendiente {
                    previa: previa.clone(),
                },
            );
            cache
                .pedidos
                .send(clave)
                .expect("decodificador de imágenes disponible");
            previa
        }
    }
}

/// Notifica una vez por carga terminada; no mantiene un reloj de redibujado.
pub fn suscripcion() -> iced::Subscription<()> {
    fn stream() -> impl iced::futures::Stream<Item = ()> {
        iced::stream::channel(1, |mut salida| async move {
            use iced::futures::SinkExt;
            let mut cambios = cache().cambios.subscribe();
            // Cubre también las cargas terminadas antes de crear la suscripción.
            let _ = salida.send(()).await;
            while cambios.changed().await.is_ok() {
                if salida.send(()).await.is_err() {
                    break;
                }
            }
        })
    }
    iced::Subscription::run(stream)
}

fn preparar(lamina: Lamina, modo: Modo) -> image::ImageResult<RgbaImage> {
    let bytes = lamina.bytes(modo);
    let derivada = bytes == crate::derivadas::INVERTIDA || bytes == crate::derivadas::IGUAL;
    let fuente = if derivada { lamina.bytes(Modo::Piedra) } else { bytes };
    let mut imagen = image::load_from_memory(fuente)?.into_rgba8();
    if bytes == crate::derivadas::INVERTIDA {
        invertir(&mut imagen);
    }
    Ok(imagen)
}

/// Negativo RGBA: invierte el color y conserva el alfa.
pub(crate) fn invertir(pixeles: &mut [u8]) {
    for pixel in pixeles.chunks_exact_mut(4) {
        for canal in &mut pixel[..3] {
            *canal = 255 - *canal;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toda_lamina_tiene_pixeles_visibles_desde_el_primer_cuadro() {
        for &lamina in crate::laminas::TODAS {
            for modo in [Modo::Tinta, Modo::Piedra] {
                let Handle::Rgba {
                    width,
                    height,
                    pixels,
                    ..
                } = lamina.previa(modo)
                else {
                    panic!("la vista previa no está preparada");
                };
                assert!(width > 1 && height > 1, "{lamina:?}");
                assert_eq!(pixels.len(), (width * height * 4) as usize);
                assert!(
                    pixels.iter().skip(3).step_by(4).any(|&a| a > 0),
                    "{lamina:?}"
                );
            }
        }
    }

    #[test]
    fn la_cache_desaloja_la_menos_reciente_y_conserva_pendientes() {
        let mut estado = Estado::default();
        let a = (Lamina::Faeton, Modo::Tinta);
        let b = (Lamina::HermesHilos, Modo::Tinta);
        let c = (Lamina::FarnesioCielo, Modo::Tinta);
        let pendiente = (Lamina::CaidaCielo, Modo::Tinta);
        estado.entradas.insert(
            pendiente,
            Entrada::Pendiente {
                previa: Handle::from_rgba(1, 1, vec![0; 4]),
            },
        );
        let handle = Handle::from_rgba(1, 1, vec![0; 4]);
        estado.insertar(a, handle.clone(), 4, 8);
        estado.insertar(b, handle.clone(), 4, 8);
        if let Some(Entrada::Lista { uso, .. }) = estado.entradas.get_mut(&a) {
            *uso = 3;
        }
        estado.insertar(c, handle, 4, 8);
        assert!(estado.entradas.contains_key(&a));
        assert!(!estado.entradas.contains_key(&b));
        assert!(matches!(
            estado.entradas.get(&pendiente),
            Some(Entrada::Pendiente { .. })
        ));
        assert_eq!(estado.bytes, 8);
    }

    #[test]
    fn faeton_conserva_el_circulo_y_la_transparencia_en_ambos_temas() {
        let positivo = preparar(Lamina::Faeton, Modo::Piedra).unwrap();
        let negativo = preparar(Lamina::Faeton, Modo::Tinta).unwrap();
        assert_eq!(positivo.dimensions(), (1900, 1910));
        for (x, y) in [(0, 0), (1899, 0), (0, 1909), (1899, 1909)] {
            assert_eq!(positivo.get_pixel(x, y).0[3], 0);
            assert_eq!(negativo.get_pixel(x, y).0[3], 0);
        }
        for (x, y) in [(950, 955), (950, 70), (950, 1840), (70, 955), (1840, 955)] {
            let p = positivo.get_pixel(x, y).0;
            let n = negativo.get_pixel(x, y).0;
            assert_eq!(p[3], 255);
            assert_eq!(n[3], 255);
            assert_eq!(p[0] as u16 + n[0] as u16, 255);
        }
    }

    #[test]
    fn tinta_derivada_comparte_la_textura_o_invierte_piedra() {
        assert_eq!(
            imagen(Lamina::Umbral, Modo::Piedra).id(),
            imagen(Lamina::Umbral, Modo::Tinta).id()
        );
        let positivo = preparar(Lamina::FortalezasValle, Modo::Piedra).unwrap();
        let negativo = preparar(Lamina::FortalezasValle, Modo::Tinta).unwrap();
        assert_eq!(positivo.dimensions(), negativo.dimensions());
        assert!(positivo.pixels().zip(negativo.pixels()).all(|(p, n)| {
            p.0[3] == n.0[3] && (0..3).all(|c| p.0[c] as u16 + n.0[c] as u16 == 255)
        }));
    }

    #[test]
    fn los_recortes_no_se_amplian_y_hermes_conserva_el_color() {
        let yelmo = preparar(Lamina::CaballeroYelmo, Modo::Tinta).unwrap();
        assert_eq!(yelmo.dimensions(), (360, 441));
        let hermes = preparar(Lamina::HermesHilos, Modo::Tinta).unwrap();
        assert_eq!(hermes.height(), LADO_MAX);
        assert!(hermes.pixels().any(|p| p.0[0] > p.0[2].saturating_add(30)));
        assert!(hermes.width() < LADO_MAX);
    }

    #[test]
    fn los_originales_mejoran_la_resolucion_sin_cambiar_el_encuadre() {
        let ciudad = preparar(Lamina::TivoliCiudad, Modo::Piedra).unwrap();
        assert_eq!(ciudad.dimensions(), (1707, 1011));
        let prodigo = preparar(Lamina::ProdigoAldea, Modo::Tinta).unwrap();
        assert_eq!(prodigo.width(), LADO_MAX);
        let relacion = prodigo.width() as f32 / prodigo.height() as f32;
        assert!((relacion - 2838.0 / 1692.0).abs() < 0.002);
    }

    #[test]
    fn pedir_la_misma_lamina_comparte_la_carga_y_los_pixeles() {
        use std::time::{Duration, Instant};
        let cache = cache();
        let mut cambios = cache.cambios.subscribe();
        let primera = imagen(Lamina::CaballeroYelmo, Modo::Piedra);
        let segunda = imagen(Lamina::CaballeroYelmo, Modo::Piedra);
        // La llamada devuelve RGBA completo o una vista previa; nunca WebP
        // que iced tenga que decodificar durante el dibujo.
        assert!(matches!(primera, Handle::Rgba { .. }));
        assert!(matches!(segunda, Handle::Rgba { .. }));
        let limite = Instant::now() + Duration::from_secs(20);
        loop {
            let lista = matches!(
                cache
                    .estado
                    .lock()
                    .unwrap()
                    .entradas
                    .get(&(Lamina::CaballeroYelmo, Modo::Piedra)),
                Some(Entrada::Lista { .. })
            );
            if lista && cambios.has_changed().unwrap() {
                break;
            }
            assert!(Instant::now() < limite, "la carga no terminó");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(cambios.has_changed().unwrap());
        cambios.borrow_and_update();
        let lista = imagen(Lamina::CaballeroYelmo, Modo::Piedra);
        assert_eq!(
            lista.id(),
            imagen(Lamina::CaballeroYelmo, Modo::Piedra).id()
        );
        assert!(matches!(
            lista,
            Handle::Rgba {
                width: 360,
                height: 441,
                ..
            }
        ));
    }
}
