use super::*;
use sha2::{Digest, Sha512};

struct Temporal(PathBuf);
impl Temporal {
    fn nueva() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path =
            std::env::temp_dir().join(format!("icaro-escritor-{:x}", u128::from_le_bytes(nonce)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha512::digest(bytes))
}

#[tokio::test]
async fn reanuda_prefijo_y_verifica_el_contenido_completo() {
    let t = Temporal::nueva();
    let path = t.0.join("contenido.part");
    let bytes = vec![73; BLOQUE * 7 + 11];
    std::fs::write(&path, &bytes[..12345]).unwrap();
    let mut out = Escritor::iniciar(
        path.clone(),
        bytes.len() as u64,
        Some(hash(&bytes)),
        true,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(out.offset(), 12345);
    out.escribir(&bytes[12345..]).await.unwrap();
    assert_eq!(out.finalizar().await.unwrap(), hash(&bytes));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[tokio::test]
async fn rechaza_prefijo_corrupto_cuerpo_corto_extra_y_hash_distinto() {
    let t = Temporal::nueva();
    for (indice, esperado, contenido) in [(0, 4, b"abc".as_slice()), (1, 2, b"abc"), (2, 3, b"xyz")]
    {
        let mut out = Escritor::iniciar(
            t.0.join(format!("{indice}.part")),
            esperado,
            Some(hash(b"abc")),
            false,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let envio = out.escribir(contenido).await;
        assert!(envio.is_err() || out.finalizar().await.is_err());
    }
    let path = t.0.join("prefijo.part");
    std::fs::write(&path, b"x").unwrap();
    let mut out = Escritor::iniciar(path, 3, Some(hash(b"abc")), true, CancellationToken::new())
        .await
        .unwrap();
    out.escribir(b"bc").await.unwrap();
    assert!(out.finalizar().await.is_err());
}

#[tokio::test]
async fn descartar_no_deja_escrituras_despues_de_liberar_el_lock() {
    let t = Temporal::nueva();
    let path = t.0.join("contenido.part");
    let mut out = Escritor::iniciar(
        path.clone(),
        (BLOQUE * 10) as u64,
        None,
        true,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    out.escribir(&vec![42; BLOQUE * 3]).await.unwrap();
    drop(out);
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .open(path.with_extension("lock"))
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if lock.try_lock().is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[tokio::test]
async fn cancelacion_no_cancela_el_token_padre() {
    let t = Temporal::nueva();
    let padre = CancellationToken::new();
    let out = Escritor::iniciar(t.0.join("a.part"), 0, None, false, padre.clone())
        .await
        .unwrap();
    out.cancelar().await;
    assert!(!padre.is_cancelled());
}

#[tokio::test]
async fn cerrar_canal_sin_fin_no_publica_exito_aunque_estan_todos_los_bytes() {
    let t = Temporal::nueva();
    let mut out = Escritor::iniciar(
        t.0.join("sin-fin.part"),
        3,
        Some(hash(b"abc")),
        false,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    out.escribir(b"abc").await.unwrap();
    out.envio.take();
    let resultado = out.resultado.take().unwrap().await.unwrap();
    assert!(matches!(resultado, Err(Error::Cancelada)));
}

#[tokio::test]
async fn cancelacion_padre_impide_finalizar_y_no_trunca_directorios() {
    let t = Temporal::nueva();
    let padre = CancellationToken::new();
    let mut out = Escritor::iniciar(
        t.0.join("cancelada.part"),
        3,
        Some(hash(b"abc")),
        false,
        padre.clone(),
    )
    .await
    .unwrap();
    out.escribir(b"abc").await.unwrap();
    padre.cancel();
    assert!(matches!(out.finalizar().await, Err(Error::Cancelada)));
    let directorio = t.0.join("directorio.part");
    std::fs::create_dir(&directorio).unwrap();
    assert!(
        Escritor::iniciar(directorio.clone(), 0, None, false, CancellationToken::new())
            .await
            .is_err()
    );
    assert!(directorio.is_dir());
}
