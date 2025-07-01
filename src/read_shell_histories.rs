use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn read_shell_histories() -> BTreeMap<String, Vec<String>> {
   let mut histories = BTreeMap::new();
   let home = "/host";
   
   let shell_configs = [
       ("bash", format!("{}/.bash_history", home)),
       ("zsh", format!("{}/.zsh_history", home)),
       ("fish", format!("{}/.local/share/fish/fish_history", home)),
       ("sh", format!("{}/.history", home)),
       ("ksh", format!("{}/.sh_history", home)),
   ];
   
   for (shell_name, hist_path) in shell_configs {
       if Path::new(&hist_path).exists() {
           match read_history_file(&hist_path, shell_name) {
               Ok(commands) => {
                   if !commands.is_empty() {
                       histories.insert(shell_name.to_string(), commands);
                   }
               }
               Err(_) => continue,
           }
       }
   }
   
   histories
}

fn read_history_file(file_path: &str, shell_type: &str) -> Result<Vec<String>, std::io::Error> {
   let content = fs::read_to_string(file_path)?;
   
   match shell_type {
       "zsh" => parse_zsh_history(&content),
       "fish" => parse_fish_history(&content),
       _ => parse_basic_history(&content),
   }
}

fn parse_basic_history(content: &str) -> Result<Vec<String>, std::io::Error> {
   Ok(content
       .lines()
       .map(|line| line.trim())
       .filter(|line| !line.is_empty())
       .map(|line| line.to_string())
       .collect())
}

fn parse_zsh_history(content: &str) -> Result<Vec<String>, std::io::Error> {
   let mut commands = Vec::new();
   
   for line in content.lines() {
       let trimmed = line.trim();
       if trimmed.is_empty() {
           continue;
       }
       
       if trimmed.starts_with(": ") {
           if let Some(semicolon_pos) = trimmed.find(';') {
               let command_start = semicolon_pos.saturating_add(1);
               if let Some(command) = trimmed.get(command_start..) {
                   if !command.is_empty() {
                       commands.push(command.to_string());
                   }
               }
           }
       } else {
           commands.push(trimmed.to_string());
       }
   }
   
   Ok(commands)
}

fn parse_fish_history(content: &str) -> Result<Vec<String>, std::io::Error> {
   let mut commands = Vec::new();
   
   for line in content.lines() {
       let trimmed = line.trim();
       if trimmed.starts_with("- cmd: ") {
           if let Some(command) = trimmed.get(7..) {
               if !command.is_empty() {
                   commands.push(command.to_string());
               }
           }
       }
   }
   
   Ok(commands)
}
