use memchr::memchr;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub type_kind: TypeKind,
    pub name: String,
    pub file_path: Rc<PathBuf>,
    pub line_number: usize,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Struct,
    Enum,
    Function,
}

pub fn get_all_files<P: AsRef<std::path::Path>>(
    dir: P,
) -> std::io::Result<BTreeMap<PathBuf, Vec<TypeDefinition>>> {
    let mut files = Vec::with_capacity(1000);
    let mut queue = VecDeque::new();
    queue.push_back(dir.as_ref().to_path_buf());

    while let Some(current_dir) = queue.pop_front() {
        if files.len() >= 1000 {
            break;
        }

        let entries = match fs::read_dir(&current_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        let mut dirs_in_level = Vec::new();

        for entry in entries {
            if files.len() >= 1000 {
                break;
            }

            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue,
            };

            if should_ignore(file_name) {
                continue;
            }

            if path.is_file() {
                let clean_path = if let Some(path_str) = path.to_str() {
                    if path_str.starts_with("/host/") {
                        PathBuf::from(&path_str[6..])
                    } else {
                        path
                    }
                } else {
                    path
                };
                files.push(clean_path);
            } else if path.is_dir() {
                dirs_in_level.push(path);
            }
        }

        for dir in dirs_in_level {
            queue.push_back(dir);
        }
    }

    let mut result = BTreeMap::new();

    let rust_files: Vec<_> = files
        .iter()
        .filter(|file_path| file_path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect();

    for file_path in &rust_files {
        let rc_path = Rc::new((*file_path).clone());
        let definitions = scan_rust_file_fast(&rc_path).unwrap_or_default();
        result.insert((*file_path).clone(), definitions);
    }

    // Add non-Rust files with empty definitions
    for file_path in files {
        if !result.contains_key(&file_path) {
            result.insert(file_path, Vec::new());
        }
    }

    Ok(result)
}

pub fn scan_rust_file_fast(
    file_path: &Rc<PathBuf>,
) -> Result<Vec<TypeDefinition>, Box<dyn std::error::Error>> {
    // Read file content as bytes first to avoid UTF-8 validation overhead
    let bytes = match fs::read(PathBuf::from("/host").join(file_path.as_ref())) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(Vec::new()), // Skip files we can't read
    };

    // Skip very large files for performance
    if bytes.len() > 1_000_000 {
        return Ok(Vec::new());
    }

    // Use byte-level parsing for better performance
    scan_with_bytes(&bytes, Rc::clone(file_path))
}

fn scan_with_bytes(
    bytes: &[u8],
    file_path: Rc<PathBuf>,
) -> Result<Vec<TypeDefinition>, Box<dyn std::error::Error>> {
    let mut definitions = Vec::with_capacity(64);
    let mut line_num = 1;
    let mut pos = 0;

    while pos < bytes.len() {
        // Use memchr to find next newline - much faster than manual iteration
        let line_end = memchr(b'\n', &bytes[pos..])
            .map(|i| pos + i)
            .unwrap_or(bytes.len());

        let line = &bytes[pos..line_end];

        // Quick rejection: skip empty lines and comments
        if !line.is_empty() {
            let first_non_ws = line.iter().position(|&b| b != b' ' && b != b'\t');
            if let Some(start) = first_non_ws {
                let trimmed = &line[start..];
                if !trimmed.starts_with(b"//") && !trimmed.starts_with(b"/*") {
                    if let Some(def) = extract_definition(trimmed, Rc::clone(&file_path), line_num)
                    {
                        definitions.push(def);

                        // Early exit if we have many definitions
                        if definitions.len() >= 300 {
                            break;
                        }
                    }
                }
            }
        }

        pos = line_end + 1;
        line_num += 1;
    }

    Ok(definitions)
}

// Note: line is already trimmed (leading whitespace removed)
fn extract_definition(
    line: &[u8],
    file_path: Rc<PathBuf>,
    line_num: usize,
) -> Option<TypeDefinition> {
    let mut i = 0;

    // Skip "pub" or "pub(...)"
    if line.len() >= 3 && &line[..3] == b"pub" {
        i = 3;

        // Skip pub(crate), pub(super), etc.
        if i < line.len() && line[i] == b'(' {
            while i < line.len() && line[i] != b')' {
                i += 1;
            }
            if i < line.len() {
                i += 1; // Skip ')'
            }
        }

        // Skip whitespace after pub
        while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
            i += 1;
        }
    }

    // Check for keywords
    if line.len() >= i + 7 && &line[i..i + 6] == b"struct" && line[i + 6] == b' ' {
        extract_identifier(&line[i + 7..]).map(|name| TypeDefinition {
            type_kind: TypeKind::Struct,
            name,
            file_path,
            line_number: line_num,
        })
    } else if line.len() >= i + 5 && &line[i..i + 4] == b"enum" && line[i + 4] == b' ' {
        extract_identifier(&line[i + 5..]).map(|name| TypeDefinition {
            type_kind: TypeKind::Enum,
            name,
            file_path,
            line_number: line_num,
        })
    } else if line.len() >= i + 3 && &line[i..i + 2] == b"fn" && line[i + 2] == b' ' {
        extract_identifier(&line[i + 3..]).map(|name| TypeDefinition {
            type_kind: TypeKind::Function,
            name,
            file_path,
            line_number: line_num,
        })
    } else {
        None
    }
}

// Extract identifier from bytes (no UTF-8 validation until we find one)
#[inline]
fn extract_identifier(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }

    // Find end of identifier (until <, {, (, ;, or whitespace)
    let end = bytes
        .iter()
        .position(|&b| {
            b == b'<'
                || b == b'{'
                || b == b'('
                || b == b';'
                || b == b' '
                || b == b'\t'
                || b == b'\n'
                || b == b'\r'
        })
        .unwrap_or(bytes.len());

    if end == 0 {
        return None;
    }

    // Convert to string (only for the identifier portion)
    let name_bytes = &bytes[..end];

    // Quick validation: first char should be alphabetic or underscore
    if !name_bytes[0].is_ascii_alphabetic() && name_bytes[0] != b'_' {
        return None;
    }

    // Convert to string
    Some(String::from_utf8_lossy(name_bytes).into_owned())
}

fn should_ignore(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | ".git"
            | ".svn"
            | ".hg"
            | "build"
            | "dist"
            | "out"
            | ".next"
            | ".nuxt"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | "vendor"
            | "deps"
            | "_build"
            | ".gradle"
            | "bin"
            | "obj"
            | ".vs"
            | ".vscode"
            | ".idea"
            | "coverage"
            | ".nyc_output"
            | "snapshots"
    )
}
