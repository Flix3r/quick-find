use regex::Regex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::Shortcut;
use std::{fs::read_dir, path::Path, sync::Mutex};

use crate::{config::Action, entry::{ActionType, Entry}};

pub struct Menu {
    pub shortcut: Shortcut,
    pub current_entries: Vec<Entry>,
    entries: Vec<Entry>,
    action: Action,
    directory: Option<String>,
    allowed_chars: String,
    match_allowed_chars_case: bool,
    allowed_regex: Option<Regex>,
    match_selection_case: bool,
    minimize_keys: bool,
    remove_extension: bool,
    command: Option<String>,
    custom_css: Option<String>,
    ignored_files: Vec<String>,
}

impl Menu {
    pub fn new(
        shortcut: Shortcut,
        entries: Vec<Entry>,
        action: Action,
        directory: Option<String>,
        allowed_chars: String,
        match_allowed_chars_case: bool,
        allowed_regex: Option<Regex>,
        match_selection_case: bool,
        minimize_keys: bool,
        remove_extension: bool,
        command: Option<String>,
        custom_css: Option<String>,
        ignored_files: Vec<String>
    ) -> Self {
        Menu {
            shortcut,
            entries,
            action,
            directory,
            allowed_chars,
            match_allowed_chars_case,
            allowed_regex,
            match_selection_case,
            minimize_keys,
            remove_extension,
            current_entries: Vec::new(),
            command,
            custom_css,
            ignored_files
        }
    }

    pub fn get_entries(&mut self, app: &AppHandle) {
        self.current_entries = match &self.directory {
            Some(dir) => {
                match read_dir(&dir) {
                    Ok(dir) => dir
                        .filter_map(|res| res.ok())
                        .filter_map(|entry| {
                            let mut name = entry.file_name()
                                .to_string_lossy().into_owned();
                            let is_dir = entry.file_type().map(|t| t.is_dir())
                                .unwrap_or(false);

                            if is_dir {
                                name.push('/');
                            } 

                            if self.ignored_files.contains(&name) {
                                return None;
                            }

                            if !is_dir && self.remove_extension {
                                name = Path::new(&name).file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(&name).to_string();
                            }

                            let full = entry.path()
                                .to_string_lossy().into_owned();

                            match &self.action {
                                Action::Open => Some(
                                    Entry::new(name, full, ActionType::Open)
                                ),
                                Action::Command => Some(Entry::new(
                                    name, 
                                    full, 
                                    ActionType::Command(
                                        self.command.clone().unwrap()
                                    )
                                ))
                            }
                        }).collect(),
                    Err(_) => {
                        crate::error(
                            app,
                            format!("Could not read directory: {}", dir)
                        );
                        Vec::new()
                    }
                }
            },
            None => Vec::new()
        };
        self.current_entries.extend(self.entries.clone());
        
        self.find_entry_selections();
    }

    pub fn emit_css(&self, app: &AppHandle) {
        if let Some(css) = &self.custom_css {
            app.emit("custom-css", css).unwrap();
        }
    }
    
    fn find_entry_selections(&mut self) {
        if self.minimize_keys {
            let mut unproductive_chars = String::from("");
            
            loop {
                let mut used_chars = String::from("");

                for entry in &mut self.current_entries {
                    let disallowed_chars = [
                        unproductive_chars.as_str(), 
                        used_chars.as_str()
                        ].concat();
                    
                    if entry.get_selection(
                        &self.allowed_chars,
                        &self.allowed_regex,
                        &disallowed_chars,
                        self.match_allowed_chars_case,
                        self.match_selection_case
                    ) { 
                        used_chars.push(
                            if self.match_selection_case { 
                                entry.selection_letter 
                            } else { 
                                entry.selection_letter.to_lowercase().next()
                                .expect(concat!(
                                    "Could not convert selection letter ",
                                    "to lowercase"
                                ))
                        })
                    } else {
                        entry.get_selection(
                            &self.allowed_chars,
                            &self.allowed_regex,
                            &unproductive_chars,
                            self.match_allowed_chars_case,
                            self.match_selection_case
                        );
                    }
                };

                let not_same = self.current_entries.iter().any(|x| {
                    x.selection_letter != 
                    self.current_entries[0].selection_letter
                });

                if !not_same {
                    unproductive_chars.push(
                        self.current_entries[0].selection_letter
                    );
                    println!(concat!(
                        "All entries attempted to use the same letter. ", 
                        "Unproductive chars are now \"{}\""
                    ), unproductive_chars);
                } else { break };
            }
        } else {
            for entry in &mut self.current_entries {
                entry.get_selection(
                    &self.allowed_chars,
                    &self.allowed_regex,
                    "",
                    self.match_allowed_chars_case,
                    self.match_selection_case
                );
            }
        }
    }

    fn filter(&mut self, in_letter: char, app: &AppHandle) -> bool {
        let letter: char;
        if !self.match_allowed_chars_case {
            letter = in_letter.to_lowercase().next()
            .expect("Could not convert filter letter to lowercase");
        } else { letter = in_letter }

        let has_match = self.current_entries.iter().any(|x| {
            if self.match_selection_case {
                x.selection_letter == letter
            } else {
                x.selection_letter.to_lowercase().next().expect(
                    "Could not convert selection letter to lowercase"
                ) == letter
            }
        });

        if !has_match { return true };

        self.current_entries.retain(|x| {
            if self.match_selection_case {
                x.selection_letter == letter
            } else {
                x.selection_letter.to_lowercase().next().expect(
                    "Could not convert selection letter to lowercase"
                ) == letter
            }
        });

        if self.current_entries.len() == 1 {
            let entry = &self.current_entries[0];
            println!("Activating entry: {}", entry.string);

            self.current_entries[0].action.activate(
                app, 
                &entry.full_string
            );

            close(app.clone());
            return false
        }

        println!("Filtered to {} entries", self.current_entries.len());

        for entry in &mut self.current_entries {
            entry.pos = entry.selection_index + 1;
        }

        self.find_entry_selections();

        true
    }
}

#[tauri::command]
pub fn filter_entries(
    app: AppHandle, 
    state_idx: State<'_, Mutex<usize>>, 
    state: State<'_, Mutex<Vec<Menu>>>, 
    in_char: char
) {
    let idx = *state_idx.lock().expect("Could not lock index mutex");
    let mut state_guard = state.lock().expect("Could not lock state mutex");
    let menu = &mut state_guard[idx];

    menu.filter(in_char, &app);
    app.emit("opened", &menu.current_entries)
        .expect("Could not emit filtered entries");
}

#[tauri::command]
pub fn close(app: AppHandle) {
    app.get_window("main").expect("Could not get window")
        .hide().expect("Could not hide window");
    *app.state::<Mutex<usize>>().lock().unwrap() = usize::MAX;
}