use std::fs::File;
use std::io::Read;
use std::path::Path;
use sha2::{Digest, Sha256};
use flate2::read::ZlibDecoder;
use crate::app::error::AppError;
use crate::services::library::db::{DbChunk, DbDocument};

pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024; // 5 MB ceiling per file
pub const TARGET_CHUNK_CHARS: usize = 1500;
pub const OVERLAP_CHARS: usize = 250;

#[derive(Debug, Clone)]
pub struct IngestedFile {
    pub document: DbDocument,
    pub chunks: Vec<DbChunk>,
}

pub struct FileIngester;

impl FileIngester {
    pub fn is_supported_extension(path: &Path) -> bool {
        match path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).as_deref() {
            Some("md") | Some("markdown") | Some("txt") | Some("text") | Some("log") => true,
            Some("rs") | Some("py") | Some("js") | Some("ts") | Some("svelte") => true,
            Some("html") | Some("css") | Some("json") | Some("csv") | Some("toml") => true,
            Some("yaml") | Some("yml") | Some("sh") | Some("sql") | Some("c") | Some("cpp") | Some("h") | Some("go") => true,
            Some("pdf") => true,
            _ => false,
        }
    }

    pub fn should_ignore_path(path: &Path) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                let name = os_str.to_string_lossy();
                if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" || name == "vendor" {
                    return true;
                }
            }
        }
        false
    }

    pub fn ingest_file(path: &Path) -> Result<IngestedFile, AppError> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| AppError::storage(format!("Cannot read metadata for {}: {e}", path.display())))?;

        if !metadata.is_file() {
            return Err(AppError::invalid_input(format!("{} is not a regular file", path.display())));
        }

        let size = metadata.len();
        if size == 0 {
            return Err(AppError::invalid_input(format!("File {} is empty", path.display())));
        }
        if size > MAX_FILE_BYTES {
            return Err(AppError::invalid_input(format!(
                "File {} exceeds maximum indexing size ({} > {} bytes)",
                path.display(), size, MAX_FILE_BYTES
            )));
        }

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let is_pdf = ext == "pdf";

        let mut file = File::open(path)
            .map_err(|e| AppError::storage(format!("Failed to open file {}: {e}", path.display())))?;
        let mut raw_bytes = Vec::with_capacity(size as usize);
        file.read_to_end(&mut raw_bytes)
            .map_err(|e| AppError::storage(format!("Failed to read file {}: {e}", path.display())))?;

        // Guard against non-PDF binary files
        if !is_pdf && Self::is_binary(&raw_bytes) {
            return Err(AppError::invalid_input(format!("File {} is a binary file", path.display())));
        }

        // Extract text
        let text_content = if is_pdf {
            Self::extract_pdf_text(&raw_bytes)?
        } else {
            String::from_utf8_lossy(&raw_bytes).to_string()
        };

        if text_content.trim().is_empty() {
            return Err(AppError::invalid_input(format!("File {} contains no extractable text", path.display())));
        }

        // Content hash
        let mut hasher = Sha256::new();
        hasher.update(&raw_bytes);
        let content_hash = format!("{:x}", hasher.finalize());

        let doc_id = format!("doc_{}", &content_hash[..16]);
        let title = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let kind = if is_pdf {
            "pdf"
        } else if ext == "md" || ext == "markdown" {
            "markdown"
        } else if matches!(ext.as_str(), "rs" | "py" | "js" | "ts" | "c" | "cpp" | "go" | "html" | "css") {
            "code"
        } else {
            "text"
        }.to_string();

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

    pub fn extract_pdf_text(bytes: &[u8]) -> Result<String, AppError> {
        let mut extracted = String::new();
        let len = bytes.len();
        let mut pos = 0;

        while pos < len {
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
                        let mut decoder = ZlibDecoder::new(stream_bytes);
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
                if b.is_ascii_alphanumeric() || b.is_ascii_whitespace() || b.is_ascii_punctuation() {
                    run.push(b);
                } else {
                    if run.len() >= 6 {
                        if let Ok(s) = std::str::from_utf8(&run) {
                            extracted.push_str(s);
                            extracted.push(' ');
                        }
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

                if let Ok(s) = std::str::from_utf8(&str_bytes) {
                    if !s.trim().is_empty() {
                        out.push_str(s);
                        out.push(' ');
                    }
                }
            } else {
                i += 1;
            }
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}
