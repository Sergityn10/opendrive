//! Almacenamiento de contenido en disco.
//!
//! Los blobs se guardan en un árbol balanceado de dos niveles
//! (`blobs/ab/cd/<uuid>`) para no meter miles de entradas en un mismo
//! directorio. El nombre del archivo en disco es una clave opaca generada por
//! el servidor: **nunca** deriva del nombre visible del archivo, de modo que
//! el usuario no puede influir en la ruta del sistema de archivos.
//!
//! Todo el acceso pasa por [`cap_std::fs::Dir`], que confina las operaciones al
//! directorio raíz y hace imposible escapar de él con `..` o rutas absolutas.

pub mod blob;
pub mod text;
pub mod thumbnail;

pub use blob::{BlobStore, BlobWriter};
pub use text::TextExtractor;
pub use thumbnail::ThumbnailService;
