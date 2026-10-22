//! Núcleo de dominio de OpenDrive.
//!
//! Este crate no depende del framework HTTP: contiene el modelo de datos, la
//! capa de persistencia, el almacenamiento de blobs en disco y la lógica de
//! autenticación. El crate `opendrive-api` lo consume para exponer la API.

pub mod auth;
pub mod config;
pub mod dav;
pub mod db;
pub mod error;
pub mod models;
pub mod storage;

pub use error::{Error, Result};
