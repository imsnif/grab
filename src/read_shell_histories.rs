use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
   pub command: String,
   pub timestamp: Option<u64>,
   pub duration: Option<u64>,
   pub working_directory: Option<String>,
   pub exit_code: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct DeduplicatedCommand {
    pub command: String,
    pub folders: Vec<String>, // All folders where this command was executed
    pub latest_timestamp: Option<u64>, // Most recent execution timestamp
    pub total_executions: usize, // Number of times this command was executed across all folders
}

pub fn read_shell_histories() -> HashMap<String, Vec<DeduplicatedCommand>> {
   let mut histories = HashMap::new();
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
               Ok(entries) => {
                   if !entries.is_empty() {
                       let deduplicated = deduplicate_commands(entries);
                       if !deduplicated.is_empty() {
                           histories.insert(shell_name.to_string(), deduplicated);
                       }
                   }
               }
               Err(_) => continue,
           }
       }
   }
   
   histories
}

fn deduplicate_commands(entries: Vec<HistoryEntry>) -> Vec<DeduplicatedCommand> {
    let mut command_map: HashMap<String, DeduplicatedCommand> = HashMap::new();
    
    for entry in entries {
        let working_dir = entry.working_directory.unwrap_or_else(|| "unknown".to_string());
        
        match command_map.get_mut(&entry.command) {
            Some(existing) => {
                // Command already exists, update it
                if !existing.folders.contains(&working_dir) {
                    existing.folders.push(working_dir);
                }
                existing.total_executions = existing.total_executions.saturating_add(1);
                
                // Update latest timestamp if this entry is more recent
                match (existing.latest_timestamp, entry.timestamp) {
                    (Some(existing_ts), Some(entry_ts)) => {
                        if entry_ts > existing_ts {
                            existing.latest_timestamp = Some(entry_ts);
                        }
                    },
                    (None, Some(entry_ts)) => {
                        existing.latest_timestamp = Some(entry_ts);
                    },
                    _ => {} // Keep existing timestamp
                }
            },
            None => {
                // New command, create entry
                command_map.insert(entry.command.clone(), DeduplicatedCommand {
                    command: entry.command,
                    folders: vec![working_dir],
                    latest_timestamp: entry.timestamp,
                    total_executions: 1,
                });
            }
        }
    }
    
    command_map.into_values().collect()
}

fn read_history_file(file_path: &str, shell_type: &str) -> Result<Vec<HistoryEntry>, std::io::Error> {
   let content = fs::read_to_string(file_path)?;
   
   match shell_type {
       "zsh" => parse_zsh_history(&content),
       "fish" => parse_fish_history(&content),
       _ => parse_basic_history(&content),
   }
}

fn parse_basic_history(content: &str) -> Result<Vec<HistoryEntry>, std::io::Error> {
   let lines: Vec<&str> = content.lines().collect();
   let mut entries = Vec::new();
   let mut i = 0;
   
   while i < lines.len() {
       if let Some(line) = lines.get(i) {
           let trimmed = line.trim();
           if trimmed.is_empty() {
               i = i.saturating_add(1);
               continue;
           }
           
           // Check if this is a timestamp line (bash with HISTTIMEFORMAT)
           if trimmed.starts_with('#') {
               if let Some(timestamp_str) = trimmed.get(1..) {
                   if let Ok(timestamp) = timestamp_str.parse::<u64>() {
                       // Next line should be the command
                       i = i.saturating_add(1);
                       if let Some(next_line) = lines.get(i) {
                           let command = next_line.trim();
                           if !command.is_empty() {
                               entries.push(HistoryEntry {
                                   command: command.to_string(),
                                   timestamp: Some(timestamp),
                                   duration: None,
                                   working_directory: None,
                                   exit_code: None,
                               });
                           }
                       }
                       i = i.saturating_add(1);
                       continue;
                   }
               }
           }
           
           // Regular command line
           entries.push(HistoryEntry {
               command: trimmed.to_string(),
               timestamp: None,
               duration: None,
               working_directory: None,
               exit_code: None,
           });
       }
       i = i.saturating_add(1);
   }
   
   Ok(entries)
}

fn parse_zsh_history(content: &str) -> Result<Vec<HistoryEntry>, std::io::Error> {
   let mut entries = Vec::new();
   
   for line in content.lines() {
       let trimmed = line.trim();
       if trimmed.is_empty() {
           continue;
       }
       
       if trimmed.starts_with(": ") {
           if let Some(semicolon_pos) = trimmed.find(';') {
               // Parse ": timestamp:duration;command"
               if let Some(timestamp_part) = trimmed.get(2..semicolon_pos) {
                   let (timestamp, duration) = if let Some(colon_pos) = timestamp_part.find(':') {
                       let timestamp = timestamp_part.get(..colon_pos)
                           .and_then(|s| s.parse().ok());
                       let duration = timestamp_part.get(colon_pos.saturating_add(1)..)
                           .and_then(|s| s.parse().ok());
                       (timestamp, duration)
                   } else {
                       // No duration, just timestamp
                       (timestamp_part.parse().ok(), None)
                   };
                   
                   if let Some(command) = trimmed.get(semicolon_pos.saturating_add(1)..) {
                       if !command.is_empty() {
                           entries.push(HistoryEntry {
                               command: command.to_string(),
                               timestamp,
                               duration,
                               working_directory: None,
                               exit_code: None,
                           });
                       }
                   }
               }
           }
       } else {
           // Fallback for non-extended format
           entries.push(HistoryEntry {
               command: trimmed.to_string(),
               timestamp: None,
               duration: None,
               working_directory: None,
               exit_code: None,
           });
       }
   }
   
   Ok(entries)
}

fn parse_fish_history(content: &str) -> Result<Vec<HistoryEntry>, std::io::Error> {
   let mut entries = Vec::new();
   let lines: Vec<&str> = content.lines().collect();
   let mut i = 0;
   
   while i < lines.len() {
       if let Some(line) = lines.get(i) {
           let trimmed = line.trim();
           
           if trimmed.starts_with("- cmd: ") {
               if let Some(command) = trimmed.get(7..) {
                   if !command.is_empty() {
                       let mut entry = HistoryEntry {
                           command: command.to_string(),
                           timestamp: None,
                           duration: None,
                           working_directory: None,
                           exit_code: None,
                       };
                       
                       // Look ahead for metadata
                       let mut j = i.saturating_add(1);
                       while j < lines.len() {
                           if let Some(meta_line) = lines.get(j) {
                               let meta_trimmed = meta_line.trim();
                               
                               if meta_trimmed.starts_with("when: ") {
                                   if let Some(timestamp_str) = meta_trimmed.get(6..) {
                                       entry.timestamp = timestamp_str.parse().ok();
                                   }
                               } else if meta_trimmed.starts_with("paths:") {
                                   // Next line might have the path
                                   if let Some(path_line) = lines.get(j.saturating_add(1)) {
                                       let path_trimmed = path_line.trim();
                                       if path_trimmed.starts_with("- ") {
                                           if let Some(path) = path_trimmed.get(2..) {
                                               entry.working_directory = Some(path.to_string());
                                           }
                                       }
                                   }
                               } else if meta_trimmed.starts_with("- cmd: ") || meta_trimmed.is_empty() {
                                   // Hit next entry or empty line, stop looking for metadata
                                   break;
                               }
                               
                               j = j.saturating_add(1);
                           } else {
                               break;
                           }
                       }
                       
                       entries.push(entry);
                       i = j.saturating_sub(1); // Will be incremented at end of loop
                   }
               }
           }
       }
       i = i.saturating_add(1);
   }
   
   Ok(entries)
}
