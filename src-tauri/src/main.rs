// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod entry;
mod menu;

use menu::Menu;
use std::sync::Mutex;
use tauri::{
    menu::{Menu as ContextMenu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager,
};
use tauri_plugin_global_shortcut::ShortcutState;

fn open_window(app: &AppHandle) -> (tauri::Window, tauri::Webview) {
    let window_size = LogicalSize::new(200, 300);
    let window = app.get_window("main").expect("Could not get app window");
    let webview = app.get_webview("main").expect("Could not get app webview");
    let cursor_pos = app
        .cursor_position()
        .expect("Could not get cursor position");
    let cursor_monitor = app
        .monitor_from_point(cursor_pos.x, cursor_pos.y)
        .expect("Could not get monitor at cursor")
        .expect("Could not find monitor at cursor");

    if !window
        .is_visible()
        .expect("Could not check if window is visible")
    {
        let monitor_pos = cursor_monitor.position();
        let monitor_size = cursor_monitor.size();

        window
            .set_position(LogicalPosition::<i32>::new(
                monitor_pos.x + monitor_size.width as i32 / 2 - window_size.width as i32 / 2,
                monitor_pos.y + monitor_size.height as i32 / 2 - window_size.height as i32 / 2,
            ))
            .expect("Could not set window position");

        window.show().expect("Could not show window");
    }

    (window, webview)
}

fn open(app: &AppHandle, menu: &mut Menu) {
    println!("Opened");

    let (window, webview) = open_window(app);

    window.set_focus().expect("Could not focus window");
    webview.set_focus().expect("Could not focus webview");
    menu.emit_css(app);
    menu.get_entries(app);
    window
        .emit("opened", &menu.current_entries)
        .expect("Could not emit initial entries");
}

fn error(app: &AppHandle, message: String) {
    println!("Error: {}", message);

    let (window, _) = open_window(app);

    // Workaround for events emitted as the app opens not being received
    tauri::async_runtime::spawn(async move {
        std::thread::sleep(std::time::Duration::from_millis(200));
        window.emit("error", message).expect("Could not emit error");
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(not(debug_assertions))]
            {
                use tauri_plugin_autostart::ManagerExt;

                app.handle().plugin(tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    None
                )).expect("Could not initialize autostart plugin");

                app.autolaunch().enable().expect("Could not enable autostart");
            }
            
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&ContextMenu::with_items(
                    app,
                    &[&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?],
                )?)
                .on_menu_event(|app, _event| app.exit(0))
                .build(app)?;

            app.manage(Mutex::new(Vec::<Menu>::new()));
            app.manage(Mutex::new(usize::MAX));

            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            for (index, menu) in app
                                .state::<Mutex<Vec<Menu>>>()
                                .lock()
                                .unwrap()
                                .iter_mut()
                                .enumerate()
                            {
                                if &menu.shortcut == shortcut {
                                    open(app, menu);
                                    *app.state::<Mutex<usize>>().lock().unwrap() = index;
                                    break;
                                }
                            }
                        }
                    })
                    .build(),
            )?;

            config::ensure_exists(app.handle());
            config::start_listening(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            menu::close,
            menu::filter_entries,
            config::open_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
