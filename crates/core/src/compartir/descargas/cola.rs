// SPDX-License-Identifier: GPL-3.0-only
//! Futuros prestados: conservan contexto Tokio y se descartan con el coordinador.
use crate::compartir::Error;
use std::{future::Future, pin::Pin, task::Poll};
use tokio_util::sync::CancellationToken;
pub(super) type Trabajo<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;
pub(super) async fn ejecutar<'a, A, T>(
    trabajos: Vec<(bool, A)>,
    propios: usize,
    cdn: usize,
    cancel: &CancellationToken,
    mut crear: impl FnMut(A) -> Trabajo<'a, T>,
    mut terminado: impl FnMut(T),
) -> Result<(), Error> {
    let (mut cola_propios, mut cola_cdn) = (
        std::collections::VecDeque::new(),
        std::collections::VecDeque::new(),
    );
    for (propio, t) in trabajos {
        if propio {
            cola_propios.push_back(t);
        } else {
            cola_cdn.push_back(t);
        }
    }
    let (mut n_propios, mut n_cdn) = (0, 0);
    let mut activos: Vec<(bool, Trabajo<'a, T>)> = Vec::new();
    loop {
        while n_propios < propios.clamp(1, 2) {
            let Some(t) = cola_propios.pop_front() else {
                break;
            };
            activos.push((true, crear(t)));
            n_propios += 1;
        }
        while n_cdn < cdn.clamp(1, 4) {
            let Some(t) = cola_cdn.pop_front() else { break };
            activos.push((false, crear(t)));
            n_cdn += 1;
        }
        if activos.is_empty() {
            return Ok(());
        }
        let (propio, resultado) = tokio::select! {
            _=cancel.cancelled()=>return Err(Error::Cancelada),
            r=std::future::poll_fn(|cx| {
                for i in 0..activos.len() {
                    if let Poll::Ready(r)=activos[i].1.as_mut().poll(cx) {
                        let (propio,t)=activos.swap_remove(i); drop(t);
                        return Poll::Ready((propio,r));
                    }
                }
                Poll::Pending
            })=>r,
        };
        if propio {
            n_propios -= 1;
        } else {
            n_cdn -= 1;
        }
        terminado(resultado?);
    }
}
