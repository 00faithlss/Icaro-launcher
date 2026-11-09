// SPDX-License-Identifier: GPL-3.0-only
use super::*;
use icaro_core::compartir::{
    firmas::RevisionFirmada,
    identidad::{AlmacenClave, Identidad},
    Error,
};
use std::sync::Mutex;
use zeroize::Zeroizing;

struct Memoria(Mutex<Option<Zeroizing<Vec<u8>>>>);
impl AlmacenClave for Memoria {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn guardar(&self, valor: &[u8]) -> Result<(), Error> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(valor.to_vec()));
        Ok(())
    }
}
#[test]
fn identidad_persistente_firma_sin_exponer_semilla() {
    let almacen = Memoria(Mutex::new(None));
    let primera = Identidad::desde_almacen(&almacen).unwrap();
    let segunda = Identidad::desde_almacen(&almacen).unwrap();
    assert_eq!(primera.clave_publica(), segunda.clave_publica());
    let firma = primera.firmar(b"mensaje");
    Identidad::verificar(&segunda.clave_publica(), b"mensaje", &firma).unwrap();
    assert!(Identidad::verificar(&segunda.clave_publica(), b"otro", &firma).is_err());
    assert_eq!(primera.huella().split(' ').count(), 6);
    let secreto: String = almacen
        .0
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert!(!format!("{primera:?}").contains(&secreto));
}

#[test]
fn credencial_corrupta_no_se_reemplaza_por_otra_identidad() {
    let almacen = Memoria(Mutex::new(Some(Zeroizing::new(vec![7; 3]))));
    assert!(Identidad::desde_almacen(&almacen).is_err());
    assert_eq!(
        almacen.0.lock().unwrap().as_ref().unwrap().as_slice(),
        [7; 3]
    );
}

#[test]
fn firma_del_manifiesto_fija_identidad_y_contenido() {
    let almacen = Memoria(Mutex::new(None));
    let identidad = Identidad::desde_almacen(&almacen).unwrap();
    let mut m = Manifiesto::leer(
        &serde_json::to_vec(&manifiesto()).unwrap(),
        &Limites::default(),
    )
    .unwrap();
    m.origen.anfitrion = identidad.clave_publica();
    let mut firmada = RevisionFirmada::crear(&identidad, &m, &Limites::default()).unwrap();
    assert_eq!(
        firmada
            .verificar(&identidad.clave_publica(), &Limites::default())
            .unwrap(),
        m
    );
    let otra = Identidad::desde_almacen(&Memoria(Mutex::new(None))).unwrap();
    assert!(firmada
        .verificar(&otra.clave_publica(), &Limites::default())
        .is_err());
    firmada.manifiesto = firmada.manifiesto.replace("1.21.1", "1.21.4");
    assert!(firmada
        .verificar(&identidad.clave_publica(), &Limites::default())
        .is_err());
}

#[test]
fn firmas_ed25519_interoperan_con_vector_rfc8032() {
    // RFC 8032, sección 7.1, TEST 1 (semilla pública de prueba).
    let semilla = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
    let bytes: Vec<_> = (0..semilla.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&semilla[i..i + 2], 16).unwrap())
        .collect();
    let identidad =
        Identidad::desde_almacen(&Memoria(Mutex::new(Some(Zeroizing::new(bytes))))).unwrap();
    assert_eq!(
        identidad.clave_publica(),
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    );
    assert_eq!(identidad.firmar(b""),"e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b");
}
