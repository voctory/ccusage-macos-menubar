use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, CheckMenuItemBuilder},
    tray::{TrayIconBuilder},
    Manager,
};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
async fn toggle_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();
    match autostart_manager.is_enabled() {
        Ok(enabled) => {
            if enabled {
                autostart_manager.disable().map_err(|e| e.to_string())?;
                Ok(false)
            } else {
                autostart_manager.enable().map_err(|e| e.to_string())?;
                Ok(true)
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![toggle_autostart, is_autostart_enabled])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Create menu items
            let title = MenuItemBuilder::with_id("title", "CC Usage")
                .enabled(false)
                .build(app)?;
            
            let opus_usage = MenuItemBuilder::with_id("opus_usage", "Opus 4: $10.23")
                .enabled(false)
                .build(app)?;
            
            let sonnet_usage = MenuItemBuilder::with_id("sonnet_usage", "Sonnet 4: $2.11")
                .enabled(false)
                .build(app)?;
            
            // Check if autostart is enabled to set initial state
            let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
            
            let launch_on_startup = CheckMenuItemBuilder::with_id("launch_on_startup", "Launch on startup")
                .checked(autostart_enabled)
                .build(app)?;
            
            let quit = MenuItemBuilder::with_id("quit", "Quit")
                .accelerator("Cmd+Q")
                .build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&title)
                .separator()
                .item(&opus_usage)
                .item(&sonnet_usage)
                .separator()
                .item(&launch_on_startup)
                .separator()
                .item(&quit)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(
                    tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?
                        .to_owned(),
                )
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "launch_on_startup" => {
                        let app_handle = app.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            match toggle_autostart(app_handle).await {
                                Ok(_new_state) => {
                                    // Menu state will be updated on next menu open
                                }
                                Err(e) => {
                                    eprintln!("Failed to toggle autostart: {}", e);
                                }
                            }
                        });
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}