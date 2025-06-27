use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use syn::{visit::Visit, File, Item, ItemEnum, ItemStruct};

#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub type_kind: TypeKind,
    pub name: String,
    pub file_path: PathBuf,
    pub line_number: usize,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Struct,
    Enum,
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
    
    for file_path in files {
        let definitions = if file_path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            scan_rust_file(&file_path).unwrap_or_default()
        } else {
            Vec::new()
        };
        result.insert(file_path, definitions);
    }
    
    Ok(result)
}

fn scan_rust_file(file_path: &PathBuf) -> Result<Vec<TypeDefinition>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(PathBuf::from("/host").join(file_path))?;
    let syntax_tree = syn::parse_file(&content)?;
    
    let line_index = LineIndex::new(&content);
    
    let mut visitor = TypeVisitor {
        definitions: Vec::new(),
        file_path: file_path.clone(),
        line_index,
        source: &content,
    };
    
    visitor.visit_file(&syntax_tree);
    Ok(visitor.definitions)
}

struct TypeVisitor<'a> {
    definitions: Vec<TypeDefinition>,
    file_path: PathBuf,
    line_index: LineIndex,
    source: &'a str,
}

impl<'ast> Visit<'ast> for TypeVisitor<'_> {
    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let line_number = self.get_line_number(&node.ident);
        self.definitions.push(TypeDefinition {
            type_kind: TypeKind::Struct,
            name: node.ident.to_string(),
            file_path: self.file_path.clone(),
            line_number,
        });
        syn::visit::visit_item_struct(self, node);
    }
    
    fn visit_item_enum(&mut self, node: &'ast ItemEnum) {
        let line_number = self.get_line_number(&node.ident);
        self.definitions.push(TypeDefinition {
            type_kind: TypeKind::Enum,
            name: node.ident.to_string(),
            file_path: self.file_path.clone(),
            line_number,
        });
        syn::visit::visit_item_enum(self, node);
    }
}

impl TypeVisitor<'_> {
    fn get_line_number(&self, ident: &syn::Ident) -> usize {
        let ident_str = ident.to_string();
        
        // Find the identifier in the source text
        let mut line = 1;
        let mut current_pos = 0;
        
        for (byte_pos, line_content) in self.source.lines().enumerate() {
            line = byte_pos + 1;
            
            // Look for struct/enum keywords followed by our identifier
            if (line_content.contains(&format!("struct {}", ident_str)) || 
                line_content.contains(&format!("enum {}", ident_str))) {
                
                // Additional validation to ensure we found the right occurrence
                let trimmed = line_content.trim();
                if trimmed.starts_with("struct ") || trimmed.starts_with("enum ") ||
                   trimmed.contains(&format!(" struct {}", ident_str)) ||
                   trimmed.contains(&format!(" enum {}", ident_str)) {
                    return line;
                }
            }
        }
        
        line // Return last line if not found (fallback)
    }
}

struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, &byte) in text.as_bytes().iter().enumerate() {
            if byte == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }
    
    fn line_number(&self, byte_offset: usize) -> usize {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(line) => line + 1,
            Err(line) => line,
        }
    }
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
