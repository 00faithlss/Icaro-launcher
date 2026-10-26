// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Marcas de las láminas Tinta que no se empaquetan: el compilado guarda la
//! marca en lugar de una segunda copia y los trabajadores de imágenes la
//! obtienen de Piedra. Compartido con `build.rs`.

/// Tinta es el negativo exacto de Piedra (mismo alfa).
pub(crate) const INVERTIDA: &[u8] = b"icaro:tinta-invertida";
/// Tinta usa la misma lámina que Piedra.
pub(crate) const IGUAL: &[u8] = b"icaro:tinta-igual";
