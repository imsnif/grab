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
    Function,    // Any function (fn or pub fn)
    PubFunction, // Public functions only (pub fn)
}

pub fn get_all_files<P: AsRef<std::path::Path>>(dir: P) -> std::io::Result<BTreeMap<PathBuf, Vec<TypeDefinition>>> {
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
    
    // Process Rust files in batches and with optimizations
    let rust_files: Vec<_> = files.iter()
        .filter(|file_path| file_path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect();
    
    eprintln!("before, rust_files: {:#?}", rust_files);
    for file_path in &rust_files {
        let rc_path = Rc::new((*file_path).clone());
        let definitions = scan_rust_file_fast(&rc_path).unwrap_or_default();
        result.insert((*file_path).clone(), definitions);
    }
    eprintln!("after");
    
    // Add non-Rust files with empty definitions
    for file_path in files {
        if !result.contains_key(&file_path) {
            result.insert(file_path, Vec::new());
        }
    }
    
    Ok(result)
}

fn scan_rust_file_fast(file_path: &Rc<PathBuf>) -> Result<Vec<TypeDefinition>, Box<dyn std::error::Error>> {
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

fn scan_with_bytes(bytes: &[u8], file_path: Rc<PathBuf>) -> Result<Vec<TypeDefinition>, Box<dyn std::error::Error>> {
    let mut definitions = Vec::new();
    let mut line_num = 1;
    let mut i = 0;

    while i < bytes.len() {
        // Skip to start of line content (skip leading whitespace)
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }

        if i >= bytes.len() {
            break;
        }

        let line_start = i;

        // Find end of line
        while i < bytes.len() && bytes[i] != b'\n' {
            i += 1;
        }

        let line = &bytes[line_start..i];

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with(b"//") || line.starts_with(b"/*") {
            if i < bytes.len() {
                i += 1; // Skip newline
            }
            line_num += 1;
            continue;
        }

        // Look for "struct ", "enum ", or "fn " keywords
        if let Some(def) = extract_definition_fast(line, Rc::clone(&file_path), line_num) {
            definitions.push(def);

            // Early exit if we have many definitions in this file
            if definitions.len() > 50 {
                break;
            }
        }

        if i < bytes.len() {
            i += 1; // Skip newline
        }
        line_num += 1;
    }

    Ok(definitions)
}

// Fast byte-level extraction of struct/enum/fn definitions
fn extract_definition_fast(line: &[u8], file_path: Rc<PathBuf>, line_num: usize) -> Option<TypeDefinition> {
    let mut i = 0;
    let mut is_pub = false;

    // Skip whitespace and check for "pub"
    while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
        i += 1;
    }

    if i + 3 <= line.len() && &line[i..i+3] == b"pub" {
        i += 3;
        if i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
            is_pub = true;
            // Skip whitespace after pub
            while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
                i += 1;
            }
        } else {
            i -= 3; // Not followed by whitespace, revert
        }
    }

    // Check for "struct "
    if i + 7 <= line.len() && &line[i..i+6] == b"struct" && line[i+6] == b' ' {
        i += 7;
        if let Some(name) = extract_identifier(&line[i..]) {
            return Some(TypeDefinition {
                type_kind: TypeKind::Struct,
                name,
                file_path,
                line_number: line_num,
            });
        }
    }

    // Check for "enum "
    if i + 5 <= line.len() && &line[i..i+4] == b"enum" && line[i+4] == b' ' {
        i += 5;
        if let Some(name) = extract_identifier(&line[i..]) {
            return Some(TypeDefinition {
                type_kind: TypeKind::Enum,
                name,
                file_path,
                line_number: line_num,
            });
        }
    }

    // Check for "fn "
    if i + 3 <= line.len() && &line[i..i+2] == b"fn" && line[i+2] == b' ' {
        i += 3;
        if let Some(name) = extract_identifier(&line[i..]) {
            let type_kind = if is_pub {
                TypeKind::PubFunction
            } else {
                TypeKind::Function
            };
            return Some(TypeDefinition {
                type_kind,
                name,
                file_path,
                line_number: line_num,
            });
        }
    }

    None
}

// Extract identifier from bytes (no UTF-8 validation until we find one)
fn extract_identifier(bytes: &[u8]) -> Option<String> {
    // Skip leading whitespace
    let mut start = 0;
    while start < bytes.len() && (bytes[start] == b' ' || bytes[start] == b'\t') {
        start += 1;
    }

    if start >= bytes.len() {
        return None;
    }

    // Find end of identifier (until <, {, (, ;, or whitespace)
    let mut end = start;
    while end < bytes.len() {
        let b = bytes[end];
        if b == b'<' || b == b'{' || b == b'(' || b == b';' || b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            break;
        }
        end += 1;
    }

    if end == start {
        return None;
    }

    // Now convert to string (only for the identifier portion)
    let name_bytes = &bytes[start..end];
    let name = String::from_utf8_lossy(name_bytes);

    // Validate identifier
    if is_valid_identifier(&name) {
        Some(name.into_owned())
    } else {
        None
    }
}

fn is_valid_identifier(name: &str) -> bool {
    !name.is_empty() 
        && name.chars().next().unwrap_or('0').is_alphabetic() 
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && name.len() < 100 // Reasonable length limit
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
// use std::collections::{BTreeMap, VecDeque};
// use std::fs;
// use std::path::PathBuf;
// use syn::{visit::Visit, File, Item, ItemEnum, ItemStruct};
// 
// #[derive(Debug, Clone)]
// pub struct TypeDefinition {
//     pub type_kind: TypeKind,
//     pub name: String,
//     pub file_path: PathBuf,
//     pub line_number: usize,
// }
// 
// #[derive(Debug, Clone)]
// pub enum TypeKind {
//     Struct,
//     Enum,
// }
// 
// pub fn get_all_files<P: AsRef<std::path::Path>>(dir: P) -> std::io::Result<BTreeMap<PathBuf, Vec<TypeDefinition>>> {
//     let mut files = Vec::with_capacity(1000);
//     let mut queue = VecDeque::new();
//     queue.push_back(dir.as_ref().to_path_buf());
//     
//     while let Some(current_dir) = queue.pop_front() {
//         if files.len() >= 1000 {
//             break;
//         }
//         
//         let entries = match fs::read_dir(&current_dir) {
//             Ok(entries) => entries,
//             Err(_) => continue,
//         };
//         
//         let mut dirs_in_level = Vec::new();
//         
//         for entry in entries {
//             if files.len() >= 1000 {
//                 break;
//             }
//             
//             let entry = match entry {
//                 Ok(entry) => entry,
//                 Err(_) => continue,
//             };
//             
//             let path = entry.path();
//             let file_name = match path.file_name().and_then(|n| n.to_str()) {
//                 Some(name) => name,
//                 None => continue,
//             };
//             
//             if should_ignore(file_name) {
//                 continue;
//             }
//             
//             if path.is_file() {
//                 let clean_path = if let Some(path_str) = path.to_str() {
//                     if path_str.starts_with("/host/") {
//                         PathBuf::from(&path_str[6..])
//                     } else {
//                         path
//                     }
//                 } else {
//                     path
//                 };
//                 files.push(clean_path);
//             } else if path.is_dir() {
//                 dirs_in_level.push(path);
//             }
//         }
//         
//         for dir in dirs_in_level {
//             queue.push_back(dir);
//         }
//     }
//     
//     let mut result = BTreeMap::new();
//     
//     for file_path in files {
//         let definitions = if file_path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
//             scan_rust_file(&file_path).unwrap_or_default()
//         } else {
//             Vec::new()
//         };
//         result.insert(file_path, definitions);
//     }
//     
//     Ok(result)
// }
// 
// fn scan_rust_file(file_path: &PathBuf) -> Result<Vec<TypeDefinition>, Box<dyn std::error::Error>> {
//     let content = fs::read_to_string(PathBuf::from("/host").join(file_path))?;
//     let syntax_tree = syn::parse_file(&content)?;
//     
//     let line_index = LineIndex::new(&content);
//     
//     let mut visitor = TypeVisitor {
//         definitions: Vec::new(),
//         file_path: file_path.clone(),
//         line_index,
//         source: &content,
//     };
//     
//     visitor.visit_file(&syntax_tree);
//     Ok(visitor.definitions)
// }
// 
// struct TypeVisitor<'a> {
//     definitions: Vec<TypeDefinition>,
//     file_path: PathBuf,
//     line_index: LineIndex,
//     source: &'a str,
// }
// 
// impl<'ast> Visit<'ast> for TypeVisitor<'_> {
//     fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
//         let line_number = self.get_line_number(&node.ident);
//         self.definitions.push(TypeDefinition {
//             type_kind: TypeKind::Struct,
//             name: node.ident.to_string(),
//             file_path: self.file_path.clone(),
//             line_number,
//         });
//         syn::visit::visit_item_struct(self, node);
//     }
//     
//     fn visit_item_enum(&mut self, node: &'ast ItemEnum) {
//         let line_number = self.get_line_number(&node.ident);
//         self.definitions.push(TypeDefinition {
//             type_kind: TypeKind::Enum,
//             name: node.ident.to_string(),
//             file_path: self.file_path.clone(),
//             line_number,
//         });
//         syn::visit::visit_item_enum(self, node);
//     }
// }
// 
// impl TypeVisitor<'_> {
//     fn get_line_number(&self, ident: &syn::Ident) -> usize {
//         let ident_str = ident.to_string();
//         
//         // Find the identifier in the source text
//         let mut line = 1;
//         let mut current_pos = 0;
//         
//         for (byte_pos, line_content) in self.source.lines().enumerate() {
//             line = byte_pos + 1;
//             
//             // Look for struct/enum keywords followed by our identifier
//             if (line_content.contains(&format!("struct {}", ident_str)) || 
//                 line_content.contains(&format!("enum {}", ident_str))) {
//                 
//                 // Additional validation to ensure we found the right occurrence
//                 let trimmed = line_content.trim();
//                 if trimmed.starts_with("struct ") || trimmed.starts_with("enum ") ||
//                    trimmed.contains(&format!(" struct {}", ident_str)) ||
//                    trimmed.contains(&format!(" enum {}", ident_str)) {
//                     return line;
//                 }
//             }
//         }
//         
//         line // Return last line if not found (fallback)
//     }
// }
// 
// struct LineIndex {
//     line_starts: Vec<usize>,
// }
// 
// impl LineIndex {
//     fn new(text: &str) -> Self {
//         let mut line_starts = vec![0];
//         for (i, &byte) in text.as_bytes().iter().enumerate() {
//             if byte == b'\n' {
//                 line_starts.push(i + 1);
//             }
//         }
//         Self { line_starts }
//     }
//     
//     fn line_number(&self, byte_offset: usize) -> usize {
//         match self.line_starts.binary_search(&byte_offset) {
//             Ok(line) => line + 1,
//             Err(line) => line,
//         }
//     }
// }
// 
// fn should_ignore(name: &str) -> bool {
//     matches!(
//         name,
//         "node_modules"
//             | "target"
//             | ".git"
//             | ".svn"
//             | ".hg"
//             | "build"
//             | "dist"
//             | "out"
//             | ".next"
//             | ".nuxt"
//             | "__pycache__"
//             | ".pytest_cache"
//             | ".mypy_cache"
//             | "vendor"
//             | "deps"
//             | "_build"
//             | ".gradle"
//             | "bin"
//             | "obj"
//             | ".vs"
//             | ".vscode"
//             | ".idea"
//             | "coverage"
//             | ".nyc_output"
//             | "snapshots"
//     )
// }
