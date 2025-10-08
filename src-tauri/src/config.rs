use regex::Regex;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_opener::OpenerExt;
use std::{path::PathBuf, str::FromStr, sync::{Mutex, MutexGuard, mpsc::channel}, time::Duration};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use crate::entry::ActionType;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub global: Global,
    pub menus: Vec<Menu>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Global {
    #[serde(default)]
    pub allowed_chars: String,
    #[serde(default)]
    pub match_allowed_chars_case: bool,
    #[serde(default = "default_allowed_regex")]
    pub allowed_regex: String,
    #[serde(default)]
    pub match_selection_case: bool,
    #[serde(default)]
    pub minimize_keys: bool,
    #[serde(default)]
    pub remove_extension: bool,
}


#[derive(Debug, Deserialize, Default)]
pub struct GlobalOverrides {
    #[serde(default)]
    pub allowed_chars: Option<String>,
    #[serde(default)]
    pub match_allowed_chars_case: Option<bool>,
    #[serde(default)]
    pub allowed_regex: Option<String>,
    #[serde(default)]
    pub match_selection_case: Option<bool>,
    #[serde(default)]
    pub minimize_keys: Option<bool>,
    #[serde(default)]
    pub remove_extension: Option<bool>,
}

fn default_allowed_regex() -> String {
    "[A-z0-9]".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Open,
    Command,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    Simple(String),
    WithCommand {
        value: String,
        command: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct Menu {
    pub hotkey: String,
    pub action: Action,
    pub directory: Option<String>,
    pub entries: Option<Vec<Entry>>,
    pub command: Option<String>,
    #[serde(rename = "global_overrides")]
    pub global_overrides: Option<GlobalOverrides>,
}

pub fn ensure_exists(app: &tauri::App) {
    let config_dir = app.path().config_dir()
        .expect("Could not get config directory")
        .join("quick-find");

    let config_path = config_dir.join("config.json");

    if !config_dir.exists() {
        println!("Creating config directory");
        std::fs::create_dir_all(&config_dir)
            .expect("Could not create config directory");
    }

    if !config_path.exists() {
        println!("Creating default config file");
        std::fs::write(&config_path, "{\n  \"$schema\": \"\",\n  \n}")
            .expect("Could not create default config file");
        app.opener().open_path(config_path.to_string_lossy(), None::<&str>)
            .expect("Could not open config");
    }
}

fn load(config_dir: &PathBuf) -> Result<Config, serde_json::Error> {
    let result = serde_json::from_str(
        &std::fs::read_to_string(config_dir.join("config.json"))
            .expect("Could not read config")
    );

    match &result {
        Ok(_) => println!("Loaded config successfully"),
        Err(_) => println!("Config invalid")
    }

    result
}

fn generate_menus(app_handle: &AppHandle, mut menus: MutexGuard<Vec<crate::Menu>>, config: Config) {
    let global_shortcut = app_handle.global_shortcut();
    global_shortcut.unregister_all()
        .expect("Could not unregister existing hotkeys");

    menus.clear();

    for menu in config.menus {
        let shortcut_opt = Shortcut::from_str(menu.hotkey.as_str());
        if shortcut_opt.is_err() {
            println!("Shortcut {} could not be parsed, the menu will be skipped", menu.hotkey);
            continue;
        };
        let shortcut = shortcut_opt.unwrap();

        let settings = match menu.global_overrides {
            Some(ref g) => &Global {
                allowed_chars: g.allowed_chars.clone().unwrap_or_else(|| config.global.allowed_chars.clone()),
                match_allowed_chars_case: g.match_allowed_chars_case.unwrap_or(config.global.match_allowed_chars_case),
                allowed_regex: g.allowed_regex.clone().unwrap_or_else(|| config.global.allowed_regex.clone()),
                match_selection_case: g.match_selection_case.unwrap_or(config.global.match_selection_case),
                minimize_keys: g.minimize_keys.unwrap_or(config.global.minimize_keys),
                remove_extension: g.remove_extension.unwrap_or(config.global.remove_extension),
            },
            None => &config.global,
        };

        let entries = menu.entries.unwrap_or_default()
            .iter().filter_map(|x| match x {
                Entry::Simple(string) => {
                    let action_type = match menu.action {
                        Action::Open => ActionType::Open,
                        Action::Command => {
                            if let Some(cmd) = &menu.command {
                                ActionType::Command(cmd.clone())
                            } else {
                                println!("Entry and menu don't have commands, skipping this entry");
                                return None;
                            }
                        }
                    };
                    Some(crate::entry::Entry::new(string.clone(), string.clone(), action_type))
                },
                Entry::WithCommand { value, command } => {
                    let action_type = match menu.action {
                        Action::Open => {
                            println!("Entry action is open yet the entry has a command, skipping this entry");
                            return None;
                        },
                        Action::Command => ActionType::Command(command.clone())
                    };
                    Some(crate::entry::Entry::new(value.clone(), value.clone(), action_type))
                }
            }).collect();

        let regex: Option<Regex>;
        if !settings.allowed_regex.is_empty() {
            let regex_res = Regex::new(settings.allowed_regex.as_str());
            if regex_res.is_err() {
                println!("Regex {} could not be parsed, the menu will be skipped", settings.allowed_regex);
                continue;
            }
            regex = Some(regex_res.unwrap());
        } else { regex = None; }

        menus.push(crate::menu::Menu::new(
            shortcut,
            entries,
            menu.action,
            menu.directory,
            settings.allowed_chars.clone(),
            settings.match_allowed_chars_case,
            regex,
            settings.match_selection_case,
            settings.minimize_keys,
            settings.remove_extension,
            menu.command
        ));

        app_handle.global_shortcut().register(shortcut)
            .expect("Could not register shortcut");
    }
}

pub fn start_listening(app_handle: AppHandle) {
    let config_dir = app_handle.path().config_dir()
        .expect("Could not get config directory")
        .join("quick-find/");

    let config_path = config_dir.join("config.json");
    
    
    std::thread::spawn(move || {
        let menus = app_handle.state::<Mutex<Vec<crate::Menu>>>();
        let (tx, rx) = channel();
        
        if let Ok(config) = load(&config_dir) {
            generate_menus(&app_handle, menus.lock().unwrap(), config);
        }

        let mut watcher: RecommendedWatcher =
            Watcher::new(
                tx, 
                notify::Config::default()
                    .with_poll_interval(Duration::from_secs(2))
            ).expect("failed to create watcher");

        watcher
            .watch(config_path.as_path(), RecursiveMode::NonRecursive)
            .expect("failed to watch file");

        loop {
            match rx.recv() {
                Ok(_) => {
                    println!("Config file changed");
                    if let Ok(config) = load(&config_dir) {
                        generate_menus(&app_handle, menus.lock().unwrap(), config);
                    }
                }
                Err(e) => println!("Watch error: {:?}", e),
            }
        }
    });
}