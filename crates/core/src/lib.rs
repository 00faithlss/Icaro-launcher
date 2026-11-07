// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Núcleo de Ícaro: manifiestos de Mojang, descargas, instancias y lanzamiento.

pub mod auth;
pub mod compartir;
pub mod cuenta;
pub mod cuentas_locales;
pub mod skin_local;
mod http;
pub mod manifest;
pub mod instancias;
pub mod exportar;
pub mod icono_instancia;
pub mod exportar_lista_mods;
pub mod modrinth;
pub mod modpacks;
pub mod servidores;
pub mod zerotier;
pub mod mods;
pub mod vault;
pub mod java;
pub mod minecraft;
pub mod lanzamiento;
pub mod opciones_minecraft;
pub mod versiones_instancia;
pub mod actualizacion;
pub mod archivos_no_verificados;

pub mod papelera;
pub mod almacenamiento;
