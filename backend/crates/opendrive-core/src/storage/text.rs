//! Extracción de texto para la búsqueda por contenido.
//!
//! Ahora mismo se indexan los formatos de texto plano, que se leen
//! localmente sin ningún servicio externo. Los formatos binarios (PDF,
//! Office) necesitarían un extractor como Apache Tika; el punto de extensión
//! es [`TextExtractor::extract`], que ya devuelve `None` para ellos.

use crate::error::Result;

/// Extensiones cuyo contenido se puede indexar directamente.
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "yaml", "yml", "toml", "ini", "cfg", "conf",
    "log", "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "java", "c", "h", "cpp",
    "hpp", "cs", "rb", "php", "sh", "bash", "zsh", "sql", "html", "htm", "css", "scss", "xml",
    "svg",
];

/// Tamaño máximo de un archivo cuyo texto se indexa (2 MiB).
///
/// Por encima de este tamaño el beneficio de la búsqueda no compensa el
/// coste de leerlo y almacenarlo.
const MAX_INDEXABLE_SIZE: u64 = 2 * 1024 * 1024;

/// Extractor de texto.
#[derive(Debug, Clone, Default)]
pub struct TextExtractor;

impl TextExtractor {
    /// Crea un extractor.
    pub fn new() -> Self {
        Self
    }

    /// Extrae el texto de un blob, según su nombre y tipo MIME.
    ///
    /// Devuelve `None` cuando el formato no es indexable.
    pub async fn extract(
        &self,
        filename: &str,
        mime_type: Option<&str>,
        data: &[u8],
    ) -> Result<Option<String>> {
        if data.is_empty() || data.len() as u64 > MAX_INDEXABLE_SIZE {
            return Ok(None);
        }

        if !is_text_format(filename, mime_type) {
            return Ok(None);
        }

        Ok(Some(decode_text(data)))
    }
}

/// `true` si el archivo es de texto plano.
fn is_text_format(filename: &str, mime_type: Option<&str>) -> bool {
    if let Some(mime) = mime_type {
        if mime.starts_with("text/") {
            return true;
        }
        if mime == "application/json" || mime == "application/xml" {
            return true;
        }
    }

    match extension(filename) {
        Some(ext) => TEXT_EXTENSIONS.contains(&ext.as_str()),
        None => false,
    }
}

/// Devuelve la extensión en minúsculas de un nombre de archivo.
fn extension(filename: &str) -> Option<String> {
    let name = filename.rsplit('/').next().unwrap_or(filename);
    let index = name.rfind('.')?;
    if index == 0 || index == name.len() - 1 {
        return None;
    }
    Some(name[index + 1..].to_lowercase())
}

/// Decodifica los bytes como texto UTF-8, descartando lo que no lo sea.
fn decode_text(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(text) => text.to_string(),
        // Un documento UTF-8 con algún byte inválido sigue siendo legible:
        // se sustituyen los bytes problemáticos en lugar de perderlo todo.
        Err(_) => String::from_utf8_lossy(data).into_owned(),
    }
}
