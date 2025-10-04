use std::process::Command;

use regex::Regex;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[derive(Clone)]
pub enum ActionType {
    Open,
    Command(String)
}

impl ActionType {
    pub fn activate(&self, app_handle: &AppHandle, string: &str) {
        match self {
            ActionType::Open => app_handle.opener().open_path(string, None::<&str>).expect("Could not open entry"),
            ActionType::Command(cmd) => {
                #[cfg(target_os = "windows")]
                let _output = Command::new("cmd")
                    .args(["/C", &cmd.replace("{}", string)])
                    .output();

                #[cfg(not(target_os = "windows"))]
                let _output = Command::new("sh")
                    .arg("-c")
                    .arg(cmd.replace("{}", string))
                    .output();
            },
        }
    }
}

#[derive(serde::Serialize, Clone)]
pub struct Entry {
    pub string: String,
    pub selection_index: usize,

    #[serde(skip_serializing)]
    pub full_string: String,
    
    #[serde(skip_serializing)]
    pub selection_letter: char,
    
    #[serde(skip_serializing)]
    pub pos: usize,

    #[serde(skip_serializing)]
    pub action: ActionType
}

impl Entry {
    pub fn new(
        string: String,
        full_string: String, 
        action: ActionType
    ) -> Self {
        Self {
            string,
            selection_letter: char::MAX,
            full_string,
            selection_index: usize::MAX,
            pos: 0,
            action
        }
    }

    pub fn get_selection(
        &mut self,
        allowed_chars: &str, 
        allowed_regex: &Option<Regex>,
        disallowed_chars: &str,
        match_case: bool, 
        match_selection_case: bool,
    ) -> bool {
        for (i, c) in self.string.char_indices().skip(self.pos) {
            if c == ' ' { continue };

            if !allowed_chars.is_empty() {
                if match_case {
                    if !allowed_chars.contains(c) { continue }
                } else {
                    if !allowed_chars.contains(c.to_lowercase().next()
                        .expect("Could not convert allowed character to lowercase")) 
                    { continue }
                }
            }
            if allowed_regex.is_some() {
                if !allowed_regex.as_ref().unwrap().is_match(&c.to_string()) {
                    continue;
                }
            }
            if match_selection_case {
                if disallowed_chars.contains(c) { continue }
            } else {
                if disallowed_chars.contains(c.to_lowercase().next()
                    .expect("Could not convert disallowed character to lowercase")) 
                { continue }
            }

            self.selection_index = i;
            self.selection_letter = c;
            return true;
        };

        return false;
    }
}