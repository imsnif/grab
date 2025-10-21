use std::path::PathBuf;
use std::rc::Rc;

mod files;

fn main() {
    let test_file = PathBuf::from("test_parse.rs");
    let rc_path = Rc::new(test_file);

    match files::scan_rust_file_fast(&rc_path) {
        Ok(definitions) => {
            println!("Found {} definitions:", definitions.len());
            for def in definitions {
                println!("  {:?} {} at line {}", def.type_kind, def.name, def.line_number);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
