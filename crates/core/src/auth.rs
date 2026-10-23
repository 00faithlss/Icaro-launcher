// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Autorización con PKCE y retorno nativo en Windows; loopback interno entre procesos.

use crate::http;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{Client, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
pub use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

mod device;
mod browser;
mod app_return;
pub use device::{begin_device, complete_device, open_device_page, DeviceGrant, DeviceInfo};
pub use app_return::{forward_app_return, register_app_return, APP_REDIRECT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnMethod { Loopback, App }

impl Default for ReturnMethod {
    fn default() -> Self {
        if cfg!(windows) { Self::App } else { Self::Loopback }
    }
}
impl ReturnMethod {
    fn redirect_uri(self, port: u16) -> String {
        match self {
            Self::Loopback => format!("http://localhost:{port}"),
            Self::App => APP_REDIRECT.to_owned(),
        }
    }
}

/// Etapas públicas del acceso; nunca contienen códigos ni credenciales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    WaitingForBrowser,
    Microsoft,
    Xbox,
    Minecraft,
    Ownership,
    Profile,
}

pub const CLIENT_ID: &str = "dcf91e3f-eb41-4373-917f-3c22ef526059";
const AUTHORIZE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPES: &str = "XboxLive.SignIn XboxLive.offline_access";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(600);
const MAX_AUTH_BODY: usize = 256 * 1024;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error("Inicio de sesión cancelado.")]
    Cancelled,
    #[error("El inicio de sesión venció. Vuelve a intentarlo.")]
    Expired,
    #[error("No se pudo preparar el inicio de sesión.")]
    Setup,
    #[error("No se pudo abrir el navegador del sistema.")]
    Browser,
    #[error("Microsoft rechazó la configuración del cliente público. Revisa «Permitir flujos de cliente público» y el Client ID de Ícaro en Azure.")]
    PublicClient,
    #[error("Microsoft rechazó la autorización. Vuelve a iniciar sesión.")]
    Denied,
    #[error("La respuesta de autenticación no corresponde a este intento.")]
    Callback,
    #[error("Tu cuenta necesita un perfil de Xbox. Revisa tu cuenta en xbox.com.")]
    XboxProfile,
    #[error(
        "Xbox no pudo autorizar esta cuenta. Revisa sus restricciones y configuración familiar."
    )]
    XboxRestricted,
    #[error("Esta cuenta no tiene acceso a Minecraft Java o todavía no tiene un perfil.")]
    NoMinecraft,
}

/// No implementa Serialize; sus credenciales solo se exponen al núcleo.
pub struct Session {
    local: bool,
    pub profile: Profile,
    pub expires_at: Instant,
    pub(crate) refresh: Zeroizing<String>,
    minecraft: Zeroizing<String>,
    xuid: Option<String>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("profile", &self.profile)
            .field("credentials", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

impl Session {
    /// Identidad local sin credenciales ni acceso a los servicios de Microsoft.
    pub fn local(nombre: &str) -> Result<Self, String> {
        if !(3..=16).contains(&nombre.len())
            || !nombre.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err("Usa entre 3 y 16 letras, números o guiones bajos, sin espacios.".into());
        }
        let mut id = <md5::Md5 as md5::Digest>::digest(format!("OfflinePlayer:{nombre}"));
        id[6] = (id[6] & 0x0f) | 0x30;
        id[8] = (id[8] & 0x3f) | 0x80;
        Ok(Self {
            local: true,
            profile: Profile { id: format!("{id:x}"), name: nombre.into() },
            expires_at: Instant::now(),
            refresh: Zeroizing::new(String::new()),
            minecraft: Zeroizing::new(String::new()),
            xuid: None,
        })
    }

    pub fn es_local(&self) -> bool { self.local }

    /// La identidad local permite ejecutar el juego, pero no autenticar en línea.
    pub fn puede_jugar(&self) -> bool {
        if self.local {
            return Self::local(&self.profile.name).is_ok_and(|s| s.profile.id == self.profile.id);
        }
        self.has_minecraft_access() && !self.needs_refresh()
    }

    /// Acceso exclusivo del núcleo al token que recibe el proceso del juego.
    pub(crate) fn minecraft_token(&self) -> &str {
        &self.minecraft
    }

    /// Identidad Xbox recibida de XSTS; no se expone a las vistas ni registros.
    pub(crate) fn xbox_user_id(&self) -> Option<&str> {
        self.xuid.as_deref()
    }

    pub fn needs_refresh(&self) -> bool {
        !self.local && Instant::now() + Duration::from_secs(60) >= self.expires_at
    }
    /// La credencial queda en el núcleo y nunca se entrega a widgets o catálogos.
    pub fn has_minecraft_access(&self) -> bool {
        !self.minecraft.is_empty()
    }
}

struct Proof {
    verifier: Zeroizing<String>,
    state: Zeroizing<String>,
}

fn random_string() -> Result<Zeroizing<String>, Error> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(bytes.as_mut()).map_err(|_| Error::Setup)?;
    Ok(Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes.as_ref())))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn authorize_url(proof: &Proof, redirect: &str) -> Url {
    let mut url = Url::parse(AUTHORIZE).expect("URL constante");
    url.query_pairs_mut().extend_pairs([
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", redirect),
        ("scope", SCOPES),
        ("state", &proof.state),
        ("code_challenge", &challenge(&proof.verifier)),
        ("code_challenge_method", "S256"),
        ("response_mode", "query"),
        ("prompt", "select_account"),
    ]);
    url
}

/// Página local sin recursos externos ni contenido recibido de Microsoft.
fn return_reply(denied: bool) -> String {
    const SCRIPT: &str = "history.replaceState(null, '', '/');";
    let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(SCRIPT.as_bytes()));
    let (title, message) = if denied {
        ("Acceso cancelado", "Vuelve a Ícaro para intentarlo de nuevo.")
    } else {
        ("Autorización recibida", "Ícaro está comprobando tu cuenta. Puedes volver al launcher y cerrar esta pestaña.")
    };
    let body = include_str!("auth/return.html").replace("__TITLE__", title)
        .replace("__MESSAGE__", message).replace("__SCRIPT__", SCRIPT);
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'sha256-{hash}'; style-src 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}
const REJECT: &str = "HTTP/1.1 400 Bad Request\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";

fn parse_callback(request: &str, state: &str, port: u16) -> Result<Zeroizing<String>, Error> {
    let mut lines = request.split("\r\n");
    let first = lines.next().ok_or(Error::Callback)?;
    let parts: Vec<_> = first.split_whitespace().collect();
    if parts.len() != 3
        || parts[0] != "GET"
        || !matches!(parts[2], "HTTP/1.0" | "HTTP/1.1")
        || !parts[1].starts_with("/?")
    {
        return Err(Error::Callback);
    }
    let hosts: Vec<_> = lines
        .filter_map(|s| s.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect();
    if hosts != [format!("localhost:{port}").as_str()] {
        return Err(Error::Callback);
    }
    let url = Url::parse(&format!("http://localhost:{}{}", port, parts[1]))
        .map_err(|_| Error::Callback)?;
    if url.path() != "/" || url.fragment().is_some() {
        return Err(Error::Callback);
    }
    let params: Vec<_> = url.query_pairs().collect();
    let values = |key| {
        params
            .iter()
            .filter(move |(k, _)| k == key)
            .map(|(_, v)| v.as_ref())
            .collect::<Vec<_>>()
    };
    if values("state") != [state] {
        return Err(Error::Callback);
    }
    if !values("error").is_empty() {
        return Err(Error::Denied);
    }
    match values("code").as_slice() {
        [code] if !code.is_empty() && code.len() <= 4096 => Ok(Zeroizing::new((*code).to_owned())),
        _ => Err(Error::Callback),
    }
}

async fn receive(
    listener: &TcpListener,
    proof: &Proof,
    port: u16,
    method: ReturnMethod,
) -> Result<Zeroizing<String>, Error> {
    loop {
        let (mut stream, peer) = listener.accept().await.map_err(|_| Error::Setup)?;
        if !peer.ip().is_loopback() {
            continue;
        }
        let parsed = tokio::time::timeout(Duration::from_secs(3), async {
            let mut request = Zeroizing::new(Vec::new());
            loop {
                let mut chunk = [0u8; 1024];
                let n = stream.read(&mut chunk).await.map_err(|_| Error::Callback)?;
                if n == 0 || request.len() + n > 8192 {
                    return Err(Error::Callback);
                }
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|s| s == b"\r\n\r\n") {
                    break;
                }
            }
            parse_callback(
                std::str::from_utf8(&request).map_err(|_| Error::Callback)?,
                &proof.state,
                port,
            )
        })
        .await;
        let result = parsed.unwrap_or(Err(Error::Callback));
        let reply = if matches!(result, Ok(_) | Err(Error::Denied)) {
            match method {
                // El proceso del enlace nativo solo necesita un acuse, nunca una página web.
                ReturnMethod::App => "HTTP/1.1 200 OK\r\nCache-Control: no-store\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
                ReturnMethod::Loopback => return_reply(matches!(result, Err(Error::Denied))),
            }
        } else {
            REJECT.to_owned()
        };
        let _ =
            tokio::time::timeout(Duration::from_secs(1), stream.write_all(reply.as_bytes())).await;
        match result {
            Err(Error::Callback) => continue,
            other => return other,
        }
    }
}

#[derive(Deserialize)]
struct MicrosoftToken {
    access_token: Zeroizing<String>,
    refresh_token: Option<Zeroizing<String>>,
}
#[derive(Deserialize)]
struct MinecraftToken {
    access_token: Zeroizing<String>,
    expires_in: u64,
}
#[derive(Deserialize)]
struct XboxToken {
    #[serde(rename = "Token")]
    token: Zeroizing<String>,
    #[serde(rename = "DisplayClaims")]
    claims: XboxClaims,
}
#[derive(Deserialize)]
struct XboxClaims {
    xui: Vec<XboxUser>,
}
#[derive(Deserialize)]
struct XboxUser {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

struct Endpoints<'a> {
    user: &'a str,
    xsts: &'a str,
    login: &'a str,
    ownership: &'a str,
    profile: &'a str,
}
const SERVICES: Endpoints<'static> = Endpoints {
    user: "https://user.auth.xboxlive.com/user/authenticate",
    xsts: "https://xsts.auth.xboxlive.com/xsts/authorize",
    login: "https://api.minecraftservices.com/authentication/login_with_xbox",
    ownership: "https://api.minecraftservices.com/entitlements/mcstore",
    profile: "https://api.minecraftservices.com/minecraft/profile",
};

async fn minecraft_session(
    client: &Client,
    msa: MicrosoftToken,
    previous_refresh: Option<Zeroizing<String>>,
    endpoints: &Endpoints<'_>,
) -> Result<Session, Error> {
    minecraft_session_with_progress(client, msa, previous_refresh, endpoints, &|_| {}).await
}

async fn minecraft_session_with_progress(
    client: &Client,
    msa: MicrosoftToken,
    previous_refresh: Option<Zeroizing<String>>,
    endpoints: &Endpoints<'_>,
    progress: &(impl Fn(Stage) + Send + Sync),
) -> Result<Session, Error> {
    let refresh = msa
        .refresh_token
        .or(previous_refresh)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Denied)?;
    progress(Stage::Xbox);
    let user: XboxToken = http::json(client.post(endpoints.user)
        .header("x-xbl-contract-version", "1")
        .json(&serde_json::json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={}", &*msa.access_token)},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"})), "Xbox", MAX_AUTH_BODY).await?;
    let xsts: XboxToken = http::json(client.post(endpoints.xsts)
        .header("x-xbl-contract-version", "1")
        .json(&serde_json::json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[&*user.token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"})), "Xbox", MAX_AUTH_BODY).await
        .map_err(|e| match e { http::Error::Status { status: 401, .. } => Error::XboxRestricted, other => other.into() })?;
    let uhs = match (user.claims.xui.as_slice(), xsts.claims.xui.as_slice()) {
        ([a], [b]) if !a.uhs.is_empty() && a.uhs == b.uhs => &b.uhs,
        _ => return Err(Error::Callback),
    };
    // Algunos relying parties sólo exponen uhs. Conserva xid cuando XSTS lo
    // entrega; nunca sustituye el identificador por uhs ni inventa uno.
    let xuid = xsts.claims.xui[0].xid.clone().filter(|s| !s.is_empty());
    if xuid.as_ref().is_some_and(|s| s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit()) || s.parse::<u64>().is_err()) {
        return Err(Error::Callback);
    }
    progress(Stage::Minecraft);
    let token: MinecraftToken = http::json(
        client.post(endpoints.login).json(
            &serde_json::json!({"identityToken":format!("XBL3.0 x={};{}", uhs, &*xsts.token)}),
        ),
        "Minecraft",
        MAX_AUTH_BODY,
    )
    .await?;
    if token.access_token.is_empty() || !(60..=86400).contains(&token.expires_in) {
        return Err(Error::Denied);
    }
    #[derive(Deserialize)]
    struct Entitlements {
        items: Vec<Entitlement>,
    }
    #[derive(Deserialize)]
    struct Entitlement {
        name: String,
    }
    progress(Stage::Ownership);
    let ownership: Entitlements = http::json(
        client
            .get(endpoints.ownership)
            .bearer_auth(&*token.access_token),
        "Minecraft",
        MAX_AUTH_BODY,
    )
    .await?;
    if !ownership
        .items
        .iter()
        .any(|i| matches!(i.name.as_str(), "game_minecraft" | "product_minecraft"))
    {
        return Err(Error::NoMinecraft);
    }
    progress(Stage::Profile);
    let profile: Profile = http::json(
        client
            .get(endpoints.profile)
            .bearer_auth(&*token.access_token),
        "Minecraft",
        MAX_AUTH_BODY,
    )
    .await
    .map_err(|e| match e {
        http::Error::Status { status: 404, .. } => Error::NoMinecraft,
        other => other.into(),
    })?;
    if profile.id.len() != 32
        || !profile.id.bytes().all(|b| b.is_ascii_hexdigit())
        || !(1..=16).contains(&profile.name.len())
        || !profile
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(Error::NoMinecraft);
    }
    Ok(Session {
        local: false,
        profile,
        expires_at: Instant::now() + Duration::from_secs(token.expires_in),
        refresh,
        minecraft: token.access_token,
        xuid,
    })
}

pub async fn login(cancel: CancellationToken) -> Result<Session, Error> {
    login_with_return(cancel, ReturnMethod::default()).await
}

pub async fn login_with_return(cancel: CancellationToken, method: ReturnMethod) -> Result<Session, Error> {
    login_with_progress(cancel, method, |_| {}).await
}

pub async fn login_with_progress(
    cancel: CancellationToken,
    method: ReturnMethod,
    progress: impl Fn(Stage) + Send + Sync,
) -> Result<Session, Error> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = tokio::time::timeout(LOGIN_TIMEOUT, async {
            if method == ReturnMethod::App { register_app_return()?; }
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.map_err(|_| Error::Setup)?;
            let port = listener.local_addr().map_err(|_| Error::Setup)?.port();
            let redirect = method.redirect_uri(port);
            let random_state = random_string()?;
            let state = match method {
                ReturnMethod::Loopback => random_state,
                ReturnMethod::App => Zeroizing::new(format!("{port}.{}", random_state.as_str())),
            };
            let proof = Proof { verifier: random_string()?, state };
            let url = authorize_url(&proof, &redirect);
            progress(Stage::WaitingForBrowser);
            tokio::task::spawn_blocking(move || browser::open(&url)).await.map_err(|_| Error::Browser)??;
            let code = receive(&listener, &proof, port, method).await?;
            drop(listener);
            progress(Stage::Microsoft);
            let client = http::client()?;
            let msa = http::json(client.post(TOKEN).form(&[("client_id", CLIENT_ID), ("grant_type", "authorization_code"), ("code", &*code), ("redirect_uri", &redirect), ("code_verifier", &*proof.verifier), ("scope", SCOPES)]), "Microsoft", MAX_AUTH_BODY).await?;
            minecraft_session_with_progress(&client, msa, None, &SERVICES, &progress).await
        }) => result.unwrap_or(Err(Error::Expired)),
    }
}

pub async fn refresh(
    refresh_token: Zeroizing<String>,
    cancel: CancellationToken,
) -> Result<Session, Error> {
    refresh_with_progress(refresh_token, cancel, |_| {}).await
}

pub async fn refresh_with_progress(
    refresh_token: Zeroizing<String>,
    cancel: CancellationToken,
    progress: impl Fn(Stage) + Send + Sync,
) -> Result<Session, Error> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = async {
            let client = http::client()?;
            progress(Stage::Microsoft);
            let msa = http::json(client.post(TOKEN).form(&[("client_id", CLIENT_ID), ("grant_type", "refresh_token"), ("refresh_token", &*refresh_token), ("scope", SCOPES)]), "Microsoft", MAX_AUTH_BODY).await?;
            minecraft_session_with_progress(&client, msa, Some(refresh_token), &SERVICES, &progress).await
        } => result,
    }
}

pub async fn renew(session: &Session, cancel: CancellationToken) -> Result<Session, Error> {
    if session.es_local() { return Err(Error::NoMinecraft); }
    refresh(session.refresh.clone(), cancel).await
}

pub async fn renew_with_progress(
    session: &Session,
    cancel: CancellationToken,
    progress: impl Fn(Stage) + Send + Sync,
) -> Result<Session, Error> {
    if session.es_local() { return Err(Error::NoMinecraft); }
    refresh_with_progress(session.refresh.clone(), cancel, progress).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn el_acceso_normal_de_windows_pide_el_retorno_nativo_con_pkce() {
        let method=ReturnMethod::default();
        let proof=Proof {verifier:Zeroizing::new("PRIVATE_VERIFIER".into()),state:Zeroizing::new("canary-state".into())};
        let url=authorize_url(&proof,&method.redirect_uri(4321));
        let params:std::collections::HashMap<_,_>=url.query_pairs().collect();
        if cfg!(windows) {
            assert_eq!(method,ReturnMethod::App);
            assert_eq!(params["redirect_uri"],APP_REDIRECT);
            assert!(!url.as_str().contains("localhost"));
        } else { assert_eq!(method,ReturnMethod::Loopback); }
        assert_eq!(params["code_challenge_method"],"S256");
        assert_eq!(params["state"],"canary-state");
    }
    #[test]
    fn return_page_is_private_and_scrubs_the_callback_address() {
        for denied in [false, true] {
            let reply = return_reply(denied);
            let (headers, body) = reply.split_once("\r\n\r\n").unwrap();
            assert!(headers.contains("Cache-Control: no-store"));
            assert!(headers.contains("Referrer-Policy: no-referrer"));
            assert!(headers.contains("default-src 'none'"));
            assert!(headers.contains(&format!("Content-Length: {}", body.len())));
            let script = body.split_once("<script>").unwrap().1.split_once("</script>").unwrap().0;
            assert_eq!(script, "history.replaceState(null, '', '/');");
            let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(script.as_bytes()));
            assert!(headers.contains(&format!("script-src 'sha256-{hash}'")));
            assert!(!body.contains("__") && !body.contains("http") && !body.contains("code="));
            assert!(body.contains(if denied { "Acceso cancelado" } else { "Autorización recibida" }));
        }
    }
    async fn exchange_case(case: u8) -> Result<Session, Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let paths = ["/user", "/xsts", "/login", "/ownership", "/profile"]
            .map(|path| format!("{url}{path}"));
        let server = tokio::spawn(async move {
            let count = match case {
                1 => 2,
                2 => 4,
                _ => 5,
            };
            for step in 0..count {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let (headers, body) = loop {
                    let mut chunk = [0u8; 4096];
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
                        let headers = String::from_utf8(bytes[..end].to_vec())
                            .unwrap()
                            .to_lowercase();
                        let length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .map(|s| s.parse::<usize>().unwrap())
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break (headers, bytes[end + 4..end + 4 + length].to_vec());
                        }
                    }
                    assert!(bytes.len() < 8192);
                };
                let response = match step {
                    0 => {
                        assert!(!headers.contains("authorization:"));
                        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(json["Properties"]["RpsTicket"], "d=CANARY_MSA");
                        serde_json::json!({"Token":"CANARY_USER", "DisplayClaims":{"xui":[{"uhs":"hash"}]}})
                    }
                    1 => {
                        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(json["RelyingParty"], "rp://api.minecraftservices.com/");
                        assert_eq!(json["Properties"]["UserTokens"], serde_json::json!(["CANARY_USER"]));
                        serde_json::json!({"Token":"CANARY_XSTS", "DisplayClaims":{"xui":[{"uhs":if case == 1 {"foreign"} else {"hash"},"xid":if case == 0 {"2533274991020393"} else {""}}]}})
                    }
                    2 => {
                        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(json["identityToken"], "XBL3.0 x=hash;CANARY_XSTS");
                        serde_json::json!({"access_token":"CANARY_GAME", "expires_in":3600})
                    }
                    3 => {
                        assert!(headers.contains("authorization: bearer canary_game"));
                        if case == 2 { serde_json::json!({"items":[]}) } else { serde_json::json!({"items":[{"name":"game_minecraft"}]}) }
                    }
                    _ => {
                        assert!(headers.contains("authorization: bearer canary_game"));
                        serde_json::json!({"id":"0123456789abcdef0123456789abcdef", "name":if case == 3 {"Bad\nName"} else {"Jugador"}})
                    }
                }.to_string();
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                    response.len()
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let endpoints = Endpoints {
            user: &paths[0],
            xsts: &paths[1],
            login: &paths[2],
            ownership: &paths[3],
            profile: &paths[4],
        };
        let msa = MicrosoftToken {
            access_token: Zeroizing::new("CANARY_MSA".into()),
            refresh_token: None,
        };
        let stages = std::sync::Mutex::new(Vec::new());
        let result = minecraft_session_with_progress(
            &Client::new(),
            msa,
            Some(Zeroizing::new("CANARY_REFRESH".into())),
            &endpoints,
            &|stage| stages.lock().unwrap().push(stage),
        )
        .await;
        server.await.unwrap();
        let expected = match case {
            1 => vec![Stage::Xbox],
            2 => vec![Stage::Xbox, Stage::Minecraft, Stage::Ownership],
            _ => vec![Stage::Xbox, Stage::Minecraft, Stage::Ownership, Stage::Profile],
        };
        assert_eq!(*stages.lock().unwrap(), expected);
        result
    }
    #[tokio::test]
    async fn validates_xbox_identity_ownership_and_profile_before_signing_in() {
        let session = exchange_case(0).await.unwrap();
        assert_eq!(session.profile.name, "Jugador");
        assert!(session.has_minecraft_access() && !session.needs_refresh());
        assert_eq!(&*session.refresh, "CANARY_REFRESH");
        assert_eq!(session.xbox_user_id(), Some("2533274991020393"));
        assert!(matches!(exchange_case(1).await, Err(Error::Callback)));
        assert!(matches!(exchange_case(2).await, Err(Error::NoMinecraft)));
        assert!(matches!(exchange_case(3).await, Err(Error::NoMinecraft)));
    }
    #[test]
    fn pkce_vector_rfc7636() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn authorization_url_does_not_include_verifier_or_client_secret() {
        let proof = Proof {
            verifier: Zeroizing::new("PRIVATE_VERIFIER".into()),
            state: Zeroizing::new("random_state".into()),
        };
        let url = authorize_url(&proof, "http://localhost:4321");
        assert!(
            !url.as_str().contains("PRIVATE_VERIFIER") && !url.as_str().contains("client_secret")
        );
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["code_challenge_method"], "S256");
        assert_eq!(params["redirect_uri"], "http://localhost:4321");
        assert_eq!(params["client_id"], CLIENT_ID);
    }
    #[test]
    fn session_debug_does_not_expose_credentials() {
        let session = Session {
            local: false,
            profile: Profile {
                id: "0123456789abcdef0123456789abcdef".into(),
                name: "Jugador".into(),
            },
            expires_at: Instant::now(),
            refresh: Zeroizing::new("CANARY_REFRESH".into()),
            minecraft: Zeroizing::new("CANARY_MINECRAFT".into()),
            xuid: Some("CANARY_XUID".into()),
        };
        let debug = format!("{session:?}");
        assert!(!debug.contains("CANARY_REFRESH") && !debug.contains("CANARY_MINECRAFT") && !debug.contains("CANARY_XUID"));
        assert!(session.needs_refresh());
    }

    #[test]
    fn xbox_claims_without_xuid_remain_compatible() {
        let old: XboxUser = serde_json::from_str(r#"{"uhs":"hash"}"#).unwrap();
        assert!(old.xid.is_none());
        let current: XboxUser = serde_json::from_str(r#"{"uhs":"hash","xid":"2533274991020393"}"#).unwrap();
        assert_eq!(current.xid.as_deref(), Some("2533274991020393"));
    }
    #[test]
    fn rejects_foreign_duplicate_and_injected_callbacks() {
        let request =
            |query: &str| format!("GET /?{query} HTTP/1.1\r\nHost: localhost:4321\r\n\r\n");
        assert_eq!(
            &*parse_callback(
                &request("state=expected&code=valid%2Bcode"),
                "expected",
                4321
            )
            .unwrap(),
            "valid+code"
        );
        for query in [
            "state=wrong&code=private",
            "state=expected&state=wrong&code=private",
            "state=expected&code=a&code=b",
            "code=private",
            "state=expected&code=",
        ] {
            assert!(matches!(
                parse_callback(&request(query), "expected", 4321),
                Err(Error::Callback)
            ));
        }
        assert!(matches!(
            parse_callback(
                &request("state=expected&error=access_denied&error_description=SECRET"),
                "expected",
                4321
            ),
            Err(Error::Denied)
        ));
        assert!(parse_callback(
            &request("state=expected&code=a").replace("localhost:4321", "attacker.example"),
            "expected",
            4321
        )
        .is_err());
        assert!(parse_callback(
            &request("state=expected&code=a").replace("GET /?", "GET https://attacker.example/?"),
            "expected",
            4321
        )
        .is_err());
    }
    #[tokio::test]
    async fn cancelled_login_does_not_open_browser() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(login(cancel).await, Err(Error::Cancelled)));
    }
    #[tokio::test]
    async fn ignores_bad_request_before_valid_callback_and_closes_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let proof = Proof {
            verifier: Zeroizing::new("verifier".into()),
            state: Zeroizing::new("expected".into()),
        };
        let send = async {
            for query in ["state=wrong&code=private", "state=expected&code=correct"] {
                let mut stream =
                    tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                        .await
                        .unwrap();
                stream
                    .write_all(
                        format!("GET /?{query} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n")
                            .as_bytes(),
                    )
                    .await
                    .unwrap();
                let mut reply = String::new();
                stream.read_to_string(&mut reply).await.unwrap();
                assert!(
                    !reply.contains("private")
                        && !reply.contains("correct")
                        && !reply.contains("expected")
                );
            }
        };
        let (code, _) = tokio::join!(receive(&listener, &proof, port, ReturnMethod::Loopback), send);
        assert_eq!(&*code.unwrap(), "correct");
        drop(listener);
        assert!(
            tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .is_err()
        );
    }
}
