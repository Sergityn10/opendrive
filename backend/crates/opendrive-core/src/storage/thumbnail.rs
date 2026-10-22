//! Generación y cache de miniaturas de imagen.
//!
//! Las miniaturas se guardan en `data/thumbs/` indexadas por el hash del
//! contenido y el tamaño solicitado. Como el hash del contenido no cambia,
//! la cache es válida para siempre: si el archivo se reemplaza, su hash
//! cambia y la miniatura se regenera sola.
//!
//! ⚠️ El crate `image` no decodifica HEIC (el formato de las fotos del
//! iPhone). Esas imágenes no tendrán miniatura; se pueden añadir con un
//! decodificador externo en el futuro.

use std::path::PathBuf;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{DynamicImage, GenericImageView, ImageEncoder};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// Tamaños de miniatura admitidos.
const ALLOWED_SIZES: [u32; 6] = [64, 128, 256, 512, 1024, 2048];

/// Tamaño por defecto de la miniatura.
pub const DEFAULT_SIZE: u32 = 256;

/// Calidad de la compresión JPEG.
const JPEG_QUALITY: u8 = 82;

/// Generador de miniaturas con cache en disco.
#[derive(Debug, Clone)]
pub struct ThumbnailService {
    root: PathBuf,
}

impl ThumbnailService {
    /// Crea el servicio sobre el directorio indicado.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Normaliza el tamaño solicitado al valor permitido más cercano por
    /// debajo, para no generar miniaturas gigantes por accidente.
    pub fn normalize_size(requested: Option<u32>) -> u32 {
        match requested {
            None => DEFAULT_SIZE,
            Some(size) => {
                let size = size.clamp(32, 4096);
                ALLOWED_SIZES
                    .iter()
                    .copied()
                    .filter(|allowed| *allowed <= size)
                    .max()
                    .unwrap_or(ALLOWED_SIZES[0])
            }
        }
    }

    /// Ruta de la miniatura en la cache.
    pub fn cache_path(&self, checksum: &str, size: u32) -> PathBuf {
        let (a, b) = checksum.split_at(2);
        self.root
            .join(a)
            .join(b)
            .join(format!("{checksum}_{size}.jpg"))
    }

    /// `true` si ya existe la miniatura en la cache.
    pub fn is_cached(&self, checksum: &str, size: u32) -> bool {
        self.cache_path(checksum, size).is_file()
    }

    /// Devuelve la ruta del archivo de la miniatura, generándola si hace falta.
    ///
    /// `data` son los bytes originales de la imagen.
    pub fn ensure(&self, checksum: &str, size: u32, data: &[u8]) -> Result<(PathBuf, bool)> {
        let path = self.cache_path(checksum, size);
        if path.is_file() {
            return Ok((path, false));
        }

        let image = image::load_from_memory(data)
            .map_err(|e| Error::Internal(format!("no se pudo decodificar la imagen: {e}")))?;

        let thumbnail = resize_to_fit(image, size)?;
        let (width, height) = thumbnail.dimensions();

        // La cache se reparte en dos niveles para no crear un directorio con
        // miles de entradas.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Se escribe en un archivo temporal y se renombra: si dos peticiones
        // generan la misma miniatura a la vez, solo queda una versión válida.
        let temp = path.with_extension(format!("tmp{}", uuid::Uuid::new_v4().simple()));

        // Las imágenes con canal alfa se guardan en PNG: JPEG no admite
        // transparencia y la pondría en negro.
        let has_alpha = thumbnail.color().has_alpha();
        let result = if has_alpha {
            let file = std::fs::File::create(&temp)?;
            PngEncoder::new(file).write_image(
                thumbnail.as_bytes(),
                width,
                height,
                thumbnail.color().into(),
            )
        } else {
            let mut buffer = Vec::new();
            let encoder = JpegEncoder::new_with_quality(&mut buffer, JPEG_QUALITY);
            let encoded = encoder.write_image(
                thumbnail.as_bytes(),
                width,
                height,
                thumbnail.color().into(),
            );
            if encoded.is_ok() {
                std::fs::write(&temp, &buffer)?;
            }
            encoded
        };

        if let Err(err) = result {
            let _ = std::fs::remove_file(&temp);
            return Err(Error::Internal(format!(
                "no se pudo codificar la miniatura: {err}"
            )));
        }

        std::fs::rename(&temp, &path)?;

        Ok((path, true))
    }

    /// Calcula el hash del contenido de un blob.
    pub fn checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}

/// Escala la imagen para que quepa en un cuadrado de `size` sin deformarla.
fn resize_to_fit(image: DynamicImage, size: u32) -> Result<DynamicImage> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(Error::Internal("la imagen no tiene dimensiones".into()));
    }

    // No se amplían las imágenes pequeñas: hacerlo solo gastaría bytes.
    if width <= size && height <= size {
        return Ok(image);
    }

    let scale = (size as f32 / width.max(height) as f32).min(1.0);
    let target_width = ((width as f32 * scale).round() as u32).max(1);
    let target_height = ((height as f32 * scale).round() as u32).max(1);

    Ok(image.resize_exact(
        target_width,
        target_height,
        image::imageops::FilterType::Lanczos3,
    ))
}
