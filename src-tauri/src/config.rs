use regex::Regex;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_opener::OpenerExt;
use std::{path::PathBuf, str::FromStr, sync::{Mutex, MutexGuard, mpsc::channel}, time::Duration};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use crate::{entry::ActionType, menu};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub global: Global,
    pub menus: Vec<Menu>,
}

#[derive(Debug, Deserialize)]
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
    #[serde(default)]
    pub custom_css: Option<String>,
    #[serde(default = "default_ignored_files")]
    pub ignored_files: Vec<String>,
}

impl Default for Global {
    fn default() -> Self {
        Global {
            allowed_chars: String::new(),
            match_allowed_chars_case: false,
            allowed_regex: default_allowed_regex(),
            match_selection_case: false,
            minimize_keys: false,
            remove_extension: false,
            custom_css: None,
            ignored_files: default_ignored_files(),
        }
    }
}

#[derive(Debug, Deserialize)]
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
    #[serde(default)]
    pub custom_css: Option<String>,
    #[serde(default)]
    pub ignored_files: Vec<String>,
}

fn default_allowed_regex() -> String {
    "[A-z0-9]".to_string()
}

fn default_ignored_files() -> Vec<String> {
    vec![
        ".DS_Store".to_string(), 
        "thumbs.db".to_string(), 
        "desktop.ini".to_string()
    ]
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

#[tauri::command]
pub fn open_config(app: AppHandle) {
    let path = app.path().config_dir()
        .expect("Could not get config directory")
        .join("quick-find")
        .join("config.json");

    if app.opener().open_path(
        path.to_string_lossy(), 
        None::<&str>
    ).is_err() {
        println!("Could not open config");
    }
}

pub fn ensure_exists(app: &AppHandle) {
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
        std::fs::write(&config_path, concat!(
            "{\n",  
            "  \"$schema\": \"https://raw.githubusercontent.com/Flix3r/quick-find/refs/heads/main/doc/config.schema.json\",\n",
            "  \"menus\": [\n",
            "    {\n",
            "      \"hotkey\": \"Ctrl+Space\",\n",
            "      \"action\": \"open\",\n",
            "      \"directory\": \"absolute/path/to/directory/\",\n",
            "    }\n",
            "  ]\n",
            "}"
        )).expect("Could not create default config file");
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
        Ok(_) => println!("Config loaded"),
        Err(e) => println!("Config invalid: {}", e)
    }

    result
}

fn generate_menus(app: &AppHandle, mut menus: MutexGuard<Vec<crate::Menu>>, config: Config) {
    let global_shortcut = app.global_shortcut();
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
                custom_css: g.custom_css.clone().or_else(|| config.global.custom_css.clone()),
                ignored_files: [config.global.ignored_files.clone(), g.ignored_files.clone()].concat(),
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
            menu.command,
            settings.custom_css.clone(),
            settings.ignored_files.clone()
        ));

        app.global_shortcut().register(shortcut)
            .expect("Could not register shortcut");
    }
}

pub fn start_listening(app_handle: &AppHandle) {
    let app = app_handle.clone();

    let config_dir = app.path().config_dir()
        .expect("Could not get config directory")
        .join("quick-find/");
    let config_path = config_dir.join("config.json");
    
    std::thread::spawn(move || {
        let menus = app.state::<Mutex<Vec<crate::Menu>>>();
        let (tx, rx) = channel();
        
        if let Ok(config) = load(&config_dir) {
            generate_menus(&app, menus.lock().unwrap(), config);
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
                        if *app.state::<Mutex<usize>>().lock().unwrap() != usize::MAX {
                            menu::close(app.clone());
                        }
                        generate_menus(&app, menus.lock().unwrap(), config);
                    }
                }
                Err(e) => println!("Watch error: {:?}", e),
            }
        }
    });
}