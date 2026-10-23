// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Código de vinculación de Microsoft. El código interno nunca sale del núcleo.

use super::{
    http, minecraft_session, CancellationToken, Client, Error, MicrosoftToken, Url, CLIENT_ID,
    MAX_AUTH_BODY, SCOPES, SERVICES,
};
use serde::Deserialize;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const DEVICE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";

#[derive(Clone)]
pub struct DeviceInfo {
    user_code: Zeroizing<String>,
    verification_uri: Url,
    expires_at: Instant,
}
impl std::fmt::Debug for DeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceInfo([REDACTED])")
    }
}
impl DeviceInfo {
    pub fn code(&self) -> &str {
        &self.user_code
    }
    pub fn seconds_left(&self) -> u64 {
        self.expires_at
            .saturating_duration_since(Instant::now())
            .as_secs()
    }
    pub fn expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
    pub fn page(&self) -> &str {
        self.verification_uri.as_str()
    }
}

/// Solo la información que necesita mostrar la interfaz es accesible.
pub struct DeviceGrant {
    info: DeviceInfo,
    device_code: Zeroizing<String>,
    interval: Duration,
}
impl std::fmt::Debug for DeviceGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceGrant([REDACTED])")
    }
}
impl DeviceGrant {
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }
}

#[derive(Deserialize)]
struct DeviceResponse {
    device_code: Zeroizing<String>,
    user_code: Zeroizing<String>,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
}
#[derive(Deserialize)]
struct ProviderError {
    error: String,
}

fn public_client_error(error: &str) -> Error {
    match error {
        "invalid_client" | "unauthorized_client" => Error::PublicClient,
        "authorization_declined" | "access_denied" => Error::Denied,
        "expired_token" => Error::Expired,
        _ => Error::Denied,
    }
}

fn grant(response: DeviceResponse) -> Result<DeviceGrant, Error> {
    let uri = Url::parse(&response.verification_uri).map_err(|_| Error::Callback)?;
    let trusted = uri.scheme() == "https"
        && uri.username().is_empty()
        && uri.password().is_none()
        && uri.fragment().is_none()
        && uri.port_or_known_default() == Some(443)
        && matches!(
            (uri.host_str(), uri.path()),
            (
                Some("microsoft.com" | "www.microsoft.com"),
                "/devicelogin" | "/link"
            ) | (
                Some("login.microsoftonline.com"),
                "/common/oauth2/deviceauth" | "/consumers/oauth2/deviceauth"
            )
        );
    if !trusted
        || response.device_code.is_empty()
        || response.device_code.len() > 16_384
        || !(4..=16).contains(&response.user_code.len())
        || !response
            .user_code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || !(60..=1800).contains(&response.expires_in)
        || !matches!(response.interval, None | Some(1..=60))
    {
        return Err(Error::Callback);
    }
    Ok(DeviceGrant {
        info: DeviceInfo {
            user_code: response.user_code,
            verification_uri: uri,
            expires_at: Instant::now() + Duration::from_secs(response.expires_in),
        },
        device_code: response.device_code,
        interval: Duration::from_secs(response.interval.unwrap_or(5)),
    })
}

async fn request_device(client: &Client, endpoint: &str) -> Result<DeviceGrant, Error> {
    let response = client
        .post(endpoint)
        .form(&[("client_id", CLIENT_ID), ("scope", SCOPES)])
        .send()
        .await
        .map_err(|_| http::Error::Network("Microsoft"))?;
    let status = response.status().as_u16();
    if status == 400 || status == 401 {
        let error: ProviderError = http::body_json(response, "Microsoft", MAX_AUTH_BODY).await?;
        return Err(public_client_error(&error.error));
    }
    if status != 200 {
        return Err(http::Error::Status {
            service: "Microsoft",
            status,
        }
        .into());
    }
    grant(http::body_json(response, "Microsoft", MAX_AUTH_BODY).await?)
}

/// Se solicita únicamente al pulsar Iniciar sesión, nunca al restaurar una cuenta.
pub async fn begin_device(cancel: CancellationToken) -> Result<DeviceGrant, Error> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = async { request_device(&http::client()?, DEVICE).await } => result,
    }
}

async fn poll(
    client: &Client,
    token_endpoint: &str,
    grant: &DeviceGrant,
) -> Result<MicrosoftToken, Error> {
    let mut interval = grant.interval;
    loop {
        // No se consulta antes del intervalo indicado por Microsoft.
        tokio::time::sleep(interval).await;
        let response = client
            .post(token_endpoint)
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &*grant.device_code),
            ])
            .send()
            .await
            .map_err(|_| http::Error::Network("Microsoft"))?;
        let status = response.status().as_u16();
        if status == 200 {
            let token: MicrosoftToken =
                http::body_json(response, "Microsoft", MAX_AUTH_BODY).await?;
            if token.access_token.is_empty() {
                return Err(Error::Denied);
            }
            return Ok(token);
        }
        if status == 400 {
            let error: ProviderError =
                http::body_json(response, "Microsoft", MAX_AUTH_BODY).await?;
            match error.error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval = interval.saturating_add(Duration::from_secs(5));
                    continue;
                }
                other => return Err(public_client_error(other)),
            }
        }
        if status == 429 {
            let retry = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60)
                .min(1800);
            interval = interval.max(Duration::from_secs(retry));
            continue;
        }
        return Err(http::Error::Status {
            service: "Microsoft",
            status,
        }
        .into());
    }
}

async fn wait_token(
    client: &Client,
    endpoint: &str,
    grant: &DeviceGrant,
    cancel: CancellationToken,
) -> Result<MicrosoftToken, Error> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = tokio::time::timeout_at(tokio::time::Instant::from_std(grant.info.expires_at), poll(client, endpoint, grant)) =>
            result.unwrap_or(Err(Error::Expired)),
    }
}

/// Mantiene las mismas verificaciones Xbox/Minecraft y la misma sesión protegida.
pub async fn complete_device(
    grant: &DeviceGrant,
    cancel: CancellationToken,
) -> Result<super::Session, Error> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = async {
            let client = http::client()?;
            let token = wait_token(&client, super::TOKEN, grant, cancel.clone()).await?;
            minecraft_session(&client, token, None, &SERVICES).await
        } => result,
    }
}

pub async fn open_device_page(info: &DeviceInfo, cancel: CancellationToken) -> Result<(), Error> {
    if info.expired() {
        return Err(Error::Expired);
    }
    let uri = info.verification_uri.clone();
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = tokio::task::spawn_blocking(move || super::browser::open(&uri)) =>
            result.map_err(|_| Error::Browser)?,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    fn response() -> DeviceResponse {
        DeviceResponse {
            device_code: Zeroizing::new("PRIVATE_DEVICE_CANARY".into()),
            user_code: Zeroizing::new("ABCD-EFGH".into()),
            verification_uri: "https://microsoft.com/devicelogin".into(),
            expires_in: 900,
            interval: Some(5),
        }
    }
    #[test]
    fn code_and_private_grant_are_redacted_and_links_validated() {
        let g = grant(response()).unwrap();
        assert!(!format!("{g:?} {:?}", g.info()).contains("ABCD-EFGH"));
        assert!(!format!("{g:?}").contains("PRIVATE_DEVICE_CANARY"));
        for url in [
            "https://microsoft.com.attacker.test/link",
            "http://microsoft.com/link",
            "https://microsoft.com@attacker.test/link",
            "https://microsoft.com:8443/link",
            "https://microsoft.com/redirect",
            "https://microsoft.com/link#evil",
        ] {
            let mut r = response();
            r.verification_uri = url.into();
            assert!(matches!(grant(r), Err(Error::Callback)));
        }
        for interval in [0, 61, u64::MAX] {
            let mut r = response();
            r.interval = Some(interval);
            assert!(matches!(grant(r), Err(Error::Callback)));
        }
    }
    async fn server(replies: Vec<(u16, String)>) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/token", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            for (status, body) in replies {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length: usize = headers
                            .lines()
                            .find_map(|s| {
                                s.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap())
                            })
                            .unwrap();
                        if bytes.len() >= end + 4 + length {
                            let form: std::collections::HashMap<_, _> = Url::parse(&format!(
                                "http://test/?{}",
                                String::from_utf8_lossy(&bytes[end + 4..])
                            ))
                            .unwrap()
                            .query_pairs()
                            .into_owned()
                            .collect();
                            assert_eq!(form["client_id"], CLIENT_ID);
                            assert!(!form.contains_key("client_secret"));
                            break;
                        }
                    }
                }
                socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        });
        (url, task)
    }
    #[tokio::test]
    async fn real_request_fields_and_response_are_used() {
        let body = r#"{"device_code":"PRIVATE_DEVICE_CANARY","user_code":"ABCD-EFGH","verification_uri":"https://microsoft.com/devicelogin","expires_in":900,"interval":5}"#;
        let (url, task) = server(vec![(200, body.into())]).await;
        let grant = request_device(&Client::new(), &url).await.unwrap();
        assert_eq!(grant.info().code(), "ABCD-EFGH");
        assert!(grant.info().seconds_left() <= 900);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn pending_then_success_does_not_finish_prematurely() {
        let (url, task) = server(vec![
            (
                400,
                r#"{"error":"authorization_pending","error_description":"PRIVATE_DESCRIPTION"}"#
                    .into(),
            ),
            (
                200,
                r#"{"access_token":"MSA_TEST","refresh_token":"REFRESH_TEST"}"#.into(),
            ),
        ])
        .await;
        let mut g = grant(response()).unwrap();
        g.interval = Duration::from_millis(2);
        let token = wait_token(&Client::new(), &url, &g, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(&*token.access_token, "MSA_TEST");
        task.await.unwrap();
    }
    #[tokio::test]
    async fn slow_down_increases_the_wait_before_retrying() {
        let (url, task) = server(vec![
            (400, r#"{"error":"slow_down"}"#.into()),
            (
                200,
                r#"{"access_token":"MSA_TEST","refresh_token":"REFRESH_TEST"}"#.into(),
            ),
        ])
        .await;
        let mut g = grant(response()).unwrap();
        g.interval = Duration::from_millis(1);
        let started = Instant::now();
        let result = wait_token(&Client::new(), &url, &g, CancellationToken::new()).await;
        assert!(result.is_ok());
        assert!(started.elapsed() >= Duration::from_secs(5));
        task.await.unwrap();
    }
    #[tokio::test]
    async fn cancellation_and_expiry_stop_before_network() {
        let mut g = grant(response()).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            wait_token(&Client::new(), "http://127.0.0.1:1", &g, cancel).await,
            Err(Error::Cancelled)
        ));
        g.info.expires_at = Instant::now();
        assert!(matches!(
            wait_token(
                &Client::new(),
                "http://127.0.0.1:1",
                &g,
                CancellationToken::new()
            )
            .await,
            Err(Error::Expired)
        ));
        assert!(matches!(
            begin_device({
                let c = CancellationToken::new();
                c.cancel();
                c
            })
            .await,
            Err(Error::Cancelled)
        ));
    }
    #[tokio::test]
    async fn provider_errors_are_sanitized() {
        for (name, expected) in [
            ("authorization_declined", Error::Denied),
            ("expired_token", Error::Expired),
            ("unauthorized_client", Error::PublicClient),
            ("unknown_PRIVATE_ERROR", Error::Denied),
        ] {
            let (url, task) = server(vec![(
                400,
                format!(r#"{{"error":"{name}","error_description":"PRIVATE_DESCRIPTION"}}"#),
            )])
            .await;
            let mut g = grant(response()).unwrap();
            g.interval = Duration::from_millis(1);
            let error = wait_token(&Client::new(), &url, &g, CancellationToken::new())
                .await
                .err()
                .unwrap();
            assert_eq!(error, expected);
            assert!(!format!("{error:?} {error}").contains("PRIVATE"));
            task.await.unwrap();
        }
    }
}
