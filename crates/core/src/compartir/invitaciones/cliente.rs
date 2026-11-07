// SPDX-License-Identifier: GPL-3.0-only
use super::{
    modelos::{Confirmacion, PaginaAccesos},
    Caducidad, Codigo, EncuentroFirmado, Error, Identidad, InvitacionCreada, InvitacionResuelta,
    SobreFirmado,
};
use crate::compartir::manifiesto::hexadecimal;
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeSet, time::Duration};

#[derive(Clone)]
pub struct ClienteInvitaciones {
    http: reqwest::Client,
    url: reqwest::Url,
    identidad: Identidad,
}
impl ClienteInvitaciones {
    pub fn nuevo(url: &str, identidad: Identidad) -> Result<Self, Error> {
        let url = reqwest::Url::parse(url).map_err(|_| Error::Datos("URL del servicio"))?;
        let local = url
            .host_str()
            .is_some_and(|h| matches!(h, "127.0.0.1" | "::1" | "[::1]"));
        if url.host_str().is_none()
            || (!matches!(url.scheme(), "https") && !(url.scheme() == "http" && local))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(Error::Datos(
                "el servicio requiere HTTPS o loopback sin credenciales",
            ));
        }
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15));
        if local {
            builder = builder.no_proxy();
        }
        Ok(Self {
            http: builder.build().map_err(|_| Error::Conexion)?,
            url,
            identidad,
        })
    }
    async fn pedir<T: DeserializeOwned>(
        &self,
        ruta: &str,
        payload: impl Serialize,
    ) -> Result<T, Error> {
        let payload = serde_json::to_string(&payload).map_err(|_| Error::Datos("solicitud"))?;
        if payload.len() > 12000 {
            return Err(Error::Limite("solicitud de invitación"));
        }
        let sobre = SobreFirmado::crear(&self.identidad, ruta, payload)?;
        let bytes = serde_json::to_vec(&sobre).map_err(|_| Error::Datos("solicitud"))?;
        if bytes.len() > 16384 {
            return Err(Error::Limite("solicitud de invitación"));
        }
        let mut response = self
            .http
            .post(
                self.url
                    .join(ruta)
                    .map_err(|_| Error::Datos("ruta del servicio"))?,
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|_| Error::Conexion)?;
        let estado = response.status().as_u16();
        let espera = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(|n| n.min(3600));
        if response.content_length().is_some_and(|n| n > 65536) {
            return Err(Error::Limite("respuesta del servicio"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Conexion)? {
            if bytes.len().saturating_add(chunk.len()) > 65536 {
                return Err(Error::Limite("respuesta del servicio"));
            }
            bytes.extend_from_slice(&chunk);
        }
        if !(200..300).contains(&estado) {
            let valor: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
            let codigo = valor
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|v| v.len() <= 80 && v.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
                .unwrap_or("temporarily_unavailable")
                .to_owned();
            return Err(Error::Servicio {
                estado,
                codigo,
                espera_segundos: espera,
            });
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::Datos("respuesta del servicio"))
    }
    pub async fn nueva(
        &self,
        publicacion: &str,
        meeting: &EncuentroFirmado,
        caducidad: Caducidad,
        cupo: u32,
    ) -> Result<InvitacionCreada, Error> {
        if cupo == 0 {
            return Err(Error::Datos("cupo de invitación"));
        }
        meeting.verificar(&self.identidad.clave_publica(), publicacion)?;
        let respuesta:InvitacionCreada=self.pedir("/v1/create",serde_json::json!({"publication":publicacion,"duration_hours":caducidad.horas(),"capacity":cupo,"meeting":meeting})).await?;
        Codigo::parse(&respuesta.code)?;
        if respuesta.capacity != cupo || respuesta.accepted != 0 {
            return Err(Error::Datos("cupo devuelto por el servicio"));
        }
        Ok(respuesta)
    }
    fn validar_resuelta(r: InvitacionResuelta) -> Result<InvitacionResuelta, Error> {
        r.meeting.verificar(&r.host, &r.publication)?;
        if r.capacity == 0 || r.accepted > r.capacity {
            return Err(Error::Datos("cupo devuelto por el servicio"));
        }
        Ok(r)
    }
    pub async fn resolver(&self, codigo: &Codigo) -> Result<InvitacionResuelta, Error> {
        Self::validar_resuelta(
            self.pedir("/v1/resolve", serde_json::json!({"code":codigo.as_str()}))
                .await?,
        )
    }
    pub async fn aceptar(&self, codigo: &Codigo) -> Result<InvitacionResuelta, Error> {
        let r = Self::validar_resuelta(
            self.pedir("/v1/claim", serde_json::json!({"code":codigo.as_str()}))
                .await?,
        )?;
        if !r.authorized {
            return Err(Error::Datos("acceso no autorizado"));
        }
        Ok(r)
    }
    pub async fn actualizar(
        &self,
        codigo: &Codigo,
        caducidad: Caducidad,
        cupo: u32,
        meeting: Option<&EncuentroFirmado>,
    ) -> Result<InvitacionResuelta, Error> {
        if cupo == 0 {
            return Err(Error::Datos("cupo de invitación"));
        }
        let mut payload = serde_json::json!({"code":codigo.as_str(),"duration_hours":caducidad.horas(),"capacity":cupo});
        if let Some(m) = meeting {
            payload["meeting"] = serde_json::to_value(m).map_err(|_| Error::Datos("encuentro"))?;
        }
        Self::validar_resuelta(self.pedir("/v1/update", payload).await?)
    }
    pub async fn revocar_codigo(&self, codigo: &Codigo) -> Result<(), Error> {
        self.confirmar(
            "/v1/revoke-code",
            serde_json::json!({"code":codigo.as_str()}),
        )
        .await
    }
    pub async fn revocar_acceso(&self, publicacion: &str, invitado: &str) -> Result<(), Error> {
        self.confirmar(
            "/v1/revoke-access",
            serde_json::json!({"publication":publicacion,"guest":invitado}),
        )
        .await
    }
    pub async fn detener(&self, publicacion: &str) -> Result<(), Error> {
        self.confirmar("/v1/stop", serde_json::json!({"publication":publicacion}))
            .await
    }
    async fn confirmar(&self, ruta: &str, payload: serde_json::Value) -> Result<(), Error> {
        let r: Confirmacion = self.pedir(ruta, payload).await?;
        if r.ok {
            Ok(())
        } else {
            Err(Error::Datos("confirmación del servicio"))
        }
    }
    pub async fn autorizados(&self, publicacion: &str) -> Result<Vec<String>, Error> {
        if !hexadecimal(publicacion, 32) {
            return Err(Error::Datos("publicación"));
        }
        let mut invitados = BTreeSet::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut payload = serde_json::json!({"publication":publicacion});
            if let Some(c) = &cursor {
                payload["after"] = c.clone().into();
            }
            let pagina: PaginaAccesos = self.pedir("/v1/authorizations", payload).await?;
            if pagina.publication != publicacion || pagina.guests.len() > 100 {
                return Err(Error::Datos("página de accesos"));
            }
            if !pagina.active {
                return Ok(vec![]);
            }
            for clave in pagina.guests {
                if !hexadecimal(&clave, 64)
                    || cursor.as_ref().is_some_and(|c| clave <= *c)
                    || !invitados.insert(clave)
                {
                    return Err(Error::Datos("identidad de acceso"));
                }
            }
            if invitados.len() > 20_000 {
                return Err(Error::Limite("accesos recibidos"));
            }
            match pagina.next {
                None => break,
                Some(next) => {
                    if !hexadecimal(&next, 64)
                        || cursor.as_ref().is_some_and(|c| next <= *c)
                        || invitados.last() != Some(&next)
                    {
                        return Err(Error::Datos("cursor de accesos"));
                    }
                    cursor = Some(next);
                }
            }
        }
        Ok(invitados.into_iter().collect())
    }
}
