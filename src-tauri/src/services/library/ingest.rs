use crate::app::error::AppError;
use crate::services::library::db::{DbChunk, DbDocument};
use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024; // 5 MB ceiling per file
pub const TARGET_CHUNK_CHARS: usize = 1500;
pub const OVERLAP_CHARS: usize = 250;
/// Most text kept from one file. A small compressed PDF can unpack to far more.
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
/// Most bytes unpacked from one compressed PDF stream.
const MAX_PDF_STREAM_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct IngestedFile {
    pub document: DbDocument,
    pub chunks: Vec<DbChunk>,
}

pub struct FileIngester;

/// File types the library can read, in lowercase. The file picker offers the
/// same list.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "md", "markdown", "txt", "text", "log", "rs", "py", "js", "ts", "svelte", "html", "css",
    "json", "csv", "toml", "yaml", "yml", "sh", "sql", "c", "cpp", "h", "go", "pdf",
];

impl FileIngester {
    pub fn is_supported_extension(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
    }

    pub fn should_ignore_path(path: &Path) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                let name = os_str.to_string_lossy();
                if name.starts_with('.')
                    || name == "node_modules"
                    || name == "target"
                    || name == "dist"
                    || name == "vendor"
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn ingest_file(path: &Path) -> Result<IngestedFile, AppError> {
        let metadata = std::fs::metadata(path).map_err(|e| {
            AppError::storage(format!("Cannot read metadata for {}: {e}", path.display()))
        })?;

        if !metadata.is_file() {
            return Err(AppError::invalid_input(format!(
                "{} is not a regular file",
                path.display()
            )));
        }

        let size = metadata.len();
        if size == 0 {
            return Err(AppError::invalid_input(format!(
                "File {} is empty",
                path.display()
            )));
        }
        if size > MAX_FILE_BYTES {
            return Err(AppError::invalid_input(format!(
                "File {} exceeds maximum indexing size ({} > {} bytes)",
                path.display(),
                size,
                MAX_FILE_BYTES
            )));
        }

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_pdf = ext == "pdf";

        let file = File::open(path).map_err(|e| {
            AppError::storage(format!("Failed to open file {}: {e}", path.display()))
        })?;
        // Read at most one byte past the limit, in case the file grew after the
        // size check above.
        let mut raw_bytes = Vec::with_capacity(size as usize);
        file.take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut raw_bytes)
            .map_err(|e| {
                AppError::storage(format!("Failed to read file {}: {e}", path.display()))
            })?;
        if raw_bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(AppError::invalid_input(format!(
                "File {} grew past the indexing limit while it was read",
                path.display()
            )));
        }

        // Guard against non-PDF binary files. UTF-16 text is full of zero
        // bytes, so a file with a byte order mark counts as text.
        let has_bom = encoding_rs::Encoding::for_bom(&raw_bytes).is_some();
        if !is_pdf && !has_bom && Self::is_binary(&raw_bytes) {
            return Err(AppError::invalid_input(format!(
                "File {} is a binary file",
                path.display()
            )));
        }

        // Extract text
        let mut text_content = if is_pdf {
            Self::pdf_text(&raw_bytes)?
        } else {
            decode_text(&raw_bytes)
        };
        truncate_at_char_boundary(&mut text_content, MAX_TEXT_BYTES);

        if text_content.trim().is_empty() {
            return Err(AppError::invalid_input(format!(
                "File {} contains no extractable text",
                path.display()
            )));
        }

        // Content hash
        let mut hasher = Sha256::new();
        hasher.update(&raw_bytes);
        let content_hash = format!("{:x}", hasher.finalize());

        // The id comes from the path, not the content. Two files with the same
        // bytes must not share one document row, or the second is never indexed.
        let mut path_hasher = Sha256::new();
        path_hasher.update(path.to_string_lossy().as_bytes());
        let path_hash = format!("{:x}", path_hasher.finalize());
        let doc_id = format!("doc_{}", &path_hash[..16]);
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let kind = if is_pdf {
            "pdf"
        } else if ext == "md" || ext == "markdown" {
            "markdown"
        } else if matches!(
            ext.as_str(),
            "rs" | "py" | "js" | "ts" | "c" | "cpp" | "go" | "html" | "css"
        ) {
            "code"
        } else {
            "text"
        }
        .to_string();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let document = DbDocument {
            id: doc_id.clone(),
            path: path.to_string_lossy().to_string(),
            title,
            content_hash,
            size,
            modified,
            indexed_at: now,
            kind,
        };

        let chunks = Self::chunk_text(&doc_id, &text_content);

        Ok(IngestedFile { document, chunks })
    }

    fn is_binary(bytes: &[u8]) -> bool {
        let sample_len = bytes.len().min(1024);
        bytes[..sample_len].contains(&0)
    }

    pub fn chunk_text(document_id: &str, text: &str) -> Vec<DbChunk> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let total_len = chars.len();

        if total_len == 0 {
            return chunks;
        }

        if total_len <= TARGET_CHUNK_CHARS {
            let chunk_text: String = chars.into_iter().collect();
            let token_count = chunk_text.split_whitespace().count();
            chunks.push(DbChunk {
                id: format!("{document_id}_chunk_0"),
                document_id: document_id.to_string(),
                chunk_index: 0,
                text: chunk_text,
                token_count,
                start_char: 0,
                end_char: total_len,
                embedding: None,
            });
            return chunks;
        }

        let mut start = 0;
        let mut chunk_index = 0;

        while start < total_len {
            let end_target = (start + TARGET_CHUNK_CHARS).min(total_len);
            let mut split_point = end_target;

            if end_target < total_len {
                // Find nearest natural boundary scanning backward
                let search_window_start = start + TARGET_CHUNK_CHARS.saturating_sub(OVERLAP_CHARS);
                let mut found_boundary = None;

                // Priority 1: Double newline (paragraph)
                for i in (search_window_start..end_target).rev() {
                    if i + 1 < total_len && chars[i] == '\n' && chars[i + 1] == '\n' {
                        found_boundary = Some(i + 2);
                        break;
                    }
                }

                // Priority 2: Sentence boundary (. or ! or ? followed by space or newline)
                if found_boundary.is_none() {
                    for i in (search_window_start..end_target).rev() {
                        if (chars[i] == '.' || chars[i] == '!' || chars[i] == '?')
                            && (i + 1 == total_len || chars[i + 1].is_whitespace())
                        {
                            found_boundary = Some(i + 1);
                            break;
                        }
                    }
                }

                // Priority 3: Word boundary (whitespace)
                if found_boundary.is_none() {
                    for i in (search_window_start..end_target).rev() {
                        if chars[i].is_whitespace() {
                            found_boundary = Some(i + 1);
                            break;
                        }
                    }
                }

                if let Some(bp) = found_boundary {
                    split_point = bp;
                }
            }

            let chunk_slice = &chars[start..split_point];
            let chunk_str: String = chunk_slice.iter().collect();
            let token_count = chunk_str.split_whitespace().count();

            chunks.push(DbChunk {
                id: format!("{document_id}_chunk_{chunk_index}"),
                document_id: document_id.to_string(),
                chunk_index,
                text: chunk_str,
                token_count,
                start_char: start,
                end_char: split_point,
                embedding: None,
            });

            chunk_index += 1;

            if split_point >= total_len {
                break;
            }

            // Advance with overlap
            let next_start = split_point.saturating_sub(OVERLAP_CHARS);
            if next_start <= start {
                start = split_point;
            } else {
                start = next_start;
            }
        }

        chunks
    }

    /// Reads a PDF's text with the pdf-extract crate, which understands fonts
    /// and text encodings. When it fails, or panics on a damaged file, the
    /// simple stream reader below gets a try.
    fn pdf_text(bytes: &[u8]) -> Result<String, AppError> {
        match std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes)) {
            Ok(Ok(text)) if !text.trim().is_empty() => Ok(text),
            Ok(Ok(_)) => Self::extract_pdf_text(bytes),
            Ok(Err(e)) => {
                log::debug!("pdf-extract could not read a PDF: {e}");
                Self::extract_pdf_text(bytes)
            }
            Err(_) => {
                log::warn!("pdf-extract stopped on a damaged PDF; using the simple reader");
                Self::extract_pdf_text(bytes)
            }
        }
    }

    /// A simple PDF reader: it unpacks content streams and keeps the text
    /// inside `( )` strings. It misses text drawn with embedded font encodings.
    pub fn extract_pdf_text(bytes: &[u8]) -> Result<String, AppError> {
        let mut extracted = String::new();
        let len = bytes.len();
        let mut pos = 0;

        while pos < len && extracted.len() < MAX_TEXT_BYTES {
            // Find "stream"
            if let Some(stream_rel) = find_subsequence(&bytes[pos..], b"stream") {
                let stream_start = pos + stream_rel + 6;
                // Skip CRLF or LF after stream
                let mut data_start = stream_start;
                if data_start < len && bytes[data_start] == b'\r' {
                    data_start += 1;
                }
                if data_start < len && bytes[data_start] == b'\n' {
                    data_start += 1;
                }

                // Check preceding dictionary for /FlateDecode
                let dict_window_start = pos.max(data_start.saturating_sub(256));
                let dict_bytes = &bytes[dict_window_start..data_start];
                let is_flate = find_subsequence(dict_bytes, b"/FlateDecode").is_some();

                // Find "endstream"
                if let Some(stream_end_rel) = find_subsequence(&bytes[data_start..], b"endstream") {
                    let stream_end = data_start + stream_end_rel;
                    let stream_bytes = &bytes[data_start..stream_end];

                    let content_bytes = if is_flate {
                        // Capped, so a tiny file cannot unpack to gigabytes.
                        let mut decoder =
                            ZlibDecoder::new(stream_bytes).take(MAX_PDF_STREAM_BYTES);
                        let mut decompressed = Vec::new();
                        if decoder.read_to_end(&mut decompressed).is_ok() {
                            decompressed
                        } else {
                            stream_bytes.to_vec()
                        }
                    } else {
                        stream_bytes.to_vec()
                    };

                    Self::parse_pdf_text_operators(&content_bytes, &mut extracted);
                    pos = stream_end + 9;
                } else {
                    pos = stream_start;
                }
            } else {
                break;
            }
        }

        if extracted.trim().is_empty() {
            // Fallback: collect any ASCII runs of length >= 4
            let mut run = Vec::new();
            for &b in bytes {
                if b.is_ascii_alphanumeric() || b.is_ascii_whitespace() || b.is_ascii_punctuation()
                {
                    run.push(b);
                } else {
                    if run.len() >= 6
                        && let Ok(s) = std::str::from_utf8(&run)
                    {
                        extracted.push_str(s);
                        extracted.push(' ');
                    }
                    run.clear();
                }
            }
        }

        Ok(extracted)
    }

    fn parse_pdf_text_operators(bytes: &[u8], out: &mut String) {
        let mut i = 0;
        let len = bytes.len();
        while i < len {
            if bytes[i] == b'(' {
                // Literal string
                i += 1;
                let mut str_bytes = Vec::new();
                let mut depth = 1;
                let mut escape = false;

                while i < len && depth > 0 {
                    let b = bytes[i];
                    if escape {
                        match b {
                            b'n' => str_bytes.push(b'\n'),
                            b'r' => str_bytes.push(b'\r'),
                            b't' => str_bytes.push(b'\t'),
                            b'\\' => str_bytes.push(b'\\'),
                            b'(' => str_bytes.push(b'('),
                            b')' => str_bytes.push(b')'),
                            _ => str_bytes.push(b),
                        }
                        escape = false;
                    } else if b == b'\\' {
                        escape = true;
                    } else if b == b'(' {
                        depth += 1;
                        str_bytes.push(b);
                    } else if b == b')' {
                        depth -= 1;
                        if depth > 0 {
                            str_bytes.push(b);
                        }
                    } else {
                        str_bytes.push(b);
                    }
                    i += 1;
                }

                if let Ok(s) = std::str::from_utf8(&str_bytes)
                    && !s.trim().is_empty()
                {
                    out.push_str(s);
                    out.push(' ');
                }
            } else {
                i += 1;
            }
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Turns file bytes into text. A byte order mark picks UTF-8 or UTF-16.
/// Without one, valid UTF-8 is used as it is, and anything else is read as
/// Windows-1252, the usual encoding of older Windows text files.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some((encoding, bom_length)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, _) = encoding.decode_without_bom_handling(&bytes[bom_length..]);
        return text.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
            text.into_owned()
        }
    }
}

/// Shortens `text` to at most `max` bytes without splitting a character.
fn truncate_at_char_boundary(text: &mut String, max: usize) {
    if text.len() <= max {
        return;
    }
    let mut cut = max;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text.truncate(cut);
}

#[cfg(test)]
mod tests {
    use super::{FileIngester, decode_text, truncate_at_char_boundary};

    #[test]
    fn text_in_common_encodings_decodes() {
        assert_eq!(decode_text("café".as_bytes()), "café");
        // UTF-8 with a byte order mark.
        assert_eq!(decode_text(b"\xEF\xBB\xBFhello"), "hello");
        // UTF-16 little endian with a byte order mark: "hi".
        assert_eq!(decode_text(b"\xFF\xFEh\x00i\x00"), "hi");
        // Windows-1252: 0xE9 is "é" and is not valid UTF-8 on its own.
        assert_eq!(decode_text(b"caf\xE9"), "café");
    }

    #[test]
    fn truncation_keeps_whole_characters() {
        let mut text = "aé".to_string(); // 'é' takes two bytes.
        truncate_at_char_boundary(&mut text, 2);
        assert_eq!(text, "a");
        let mut short = "abc".to_string();
        truncate_at_char_boundary(&mut short, 10);
        assert_eq!(short, "abc");
    }

    #[test]
    fn utf16_files_are_text_not_binary() {
        let dir = std::env::temp_dir().join(format!("la-ingest-utf16-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("notes.txt");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "meeting notes".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&file, bytes).unwrap();

        let ingested = FileIngester::ingest_file(&file).unwrap();
        assert!(ingested.chunks[0].text.contains("meeting notes"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn identical_files_get_their_own_document_ids() {
        let dir = std::env::temp_dir().join(format!("la-ingest-ids-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let first = dir.join("first.txt");
        let second = dir.join("second.txt");
        std::fs::write(&first, "the same words in both files").unwrap();
        std::fs::write(&second, "the same words in both files").unwrap();

        let a = FileIngester::ingest_file(&first).unwrap();
        let b = FileIngester::ingest_file(&second).unwrap();
        assert_ne!(a.document.id, b.document.id);
        assert_eq!(a.document.content_hash, b.document.content_hash);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
