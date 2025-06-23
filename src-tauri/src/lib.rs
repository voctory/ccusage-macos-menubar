use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn show_window<R: Runtime>(app: tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, show_window])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let quit = MenuItemBuilder::with_id("quit", "Quit")
                .accelerator("Cmd+Q")
                .build(app)?;
            let show = MenuItemBuilder::with_id("show", "Show").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            let _tray = TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?.to_owned())
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        } else {
                            let _window = tauri::WebviewWindowBuilder::new(
                                app,
                                "main",
                                tauri::WebviewUrl::App("index.html".into()),
                            )
                            .title("CC Usage")
                            .inner_size(400.0, 300.0)
                            .build()
                            .unwrap();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        } else {
                            let _window = tauri::WebviewWindowBuilder::new(
                                app,
                                "main",
                                tauri::WebviewUrl::App("index.html".into()),
                            )
                            .title("CC Usage")
                            .inner_size(400.0, 300.0)
                            .build()
                            .unwrap();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}