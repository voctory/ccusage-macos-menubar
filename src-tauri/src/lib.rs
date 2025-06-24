use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, CheckMenuItemBuilder},
    tray::{TrayIconBuilder},
    Manager,
};
use tauri_plugin_autostart::ManagerExt;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Instant;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelBreakdown {
    #[serde(rename = "modelName")]
    model_name: String,
    #[serde(rename = "inputTokens")]
    input_tokens: u64,
    #[serde(rename = "outputTokens")]
    output_tokens: u64,
    #[serde(rename = "cacheCreationTokens")]
    cache_creation_tokens: u64,
    #[serde(rename = "cacheReadTokens")]
    cache_read_tokens: u64,
    cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyUsage {
    date: String,
    #[serde(rename = "inputTokens")]
    input_tokens: u64,
    #[serde(rename = "outputTokens")]
    output_tokens: u64,
    #[serde(rename = "cacheCreationTokens")]
    cache_creation_tokens: u64,
    #[serde(rename = "cacheReadTokens")]
    cache_read_tokens: u64,
    #[serde(rename = "totalTokens")]
    total_tokens: u64,
    #[serde(rename = "totalCost")]
    total_cost: f64,
    #[serde(rename = "modelsUsed")]
    models_used: Vec<String>,
    #[serde(rename = "modelBreakdowns")]
    model_breakdowns: Vec<ModelBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CcusageResponse {
    daily: Vec<DailyUsage>,
    totals: serde_json::Value,
}

#[derive(Debug, Clone)]
struct UsageCache {
    data: Option<CcusageResponse>,
    last_updated: Option<Instant>,
    last_error: Option<String>,
}

static USAGE_CACHE: Mutex<UsageCache> = Mutex::new(UsageCache {
    data: None,
    last_updated: None,
    last_error: None,
});

#[tauri::command]
async fn fetch_usage_data() -> Result<CcusageResponse, String> {
    let output = Command::new("npx")
        .args(&["ccusage@latest", "daily", "--json", "--breakdown"])
        .output()
        .await
        .map_err(|e| format!("Failed to run ccusage: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ccusage command failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: CcusageResponse = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse ccusage output: {}", e))?;

    // Update cache
    {
        let mut cache = USAGE_CACHE.lock().unwrap();
        cache.data = Some(response.clone());
        cache.last_updated = Some(Instant::now());
        cache.last_error = None;
    }

    Ok(response)
}

#[tauri::command]
async fn get_cached_usage_data() -> Result<Option<CcusageResponse>, String> {
    let cache = USAGE_CACHE.lock().unwrap();
    Ok(cache.data.clone())
}

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

fn format_model_name(model_name: &str) -> String {
    match model_name {
        "claude-opus-4-20250514" => "Opus 4".to_string(),
        "claude-sonnet-4-20250514" => "Sonnet 4".to_string(),
        "claude-3-5-sonnet-20241022" => "Sonnet 3.5".to_string(),
        "claude-3-haiku-20240307" => "Haiku".to_string(),
        _ => {
            // Extract model name from full identifier
            if model_name.contains("opus") {
                "Opus".to_string()
            } else if model_name.contains("sonnet") {
                "Sonnet".to_string()
            } else if model_name.contains("haiku") {
                "Haiku".to_string()
            } else {
                model_name.to_string()
            }
        }
    }
}

async fn build_menu_with_usage_data(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    // Try to fetch fresh data first, fall back to cache
    let usage_result = fetch_usage_data().await;
    let usage_data = match usage_result {
        Ok(data) => Some(data),
        Err(_) => {
            // Try to get cached data
            get_cached_usage_data().await.unwrap_or(None)
        }
    };

    let title = MenuItemBuilder::with_id("title", "CC Usage - Today")
        .enabled(false)
        .build(app)?;

    let mut menu_builder = MenuBuilder::new(app).item(&title).separator();

    match usage_data {
        Some(data) => {
            if let Some(today) = data.daily.first() {
                if today.model_breakdowns.is_empty() {
                    let no_data = MenuItemBuilder::with_id("no_data", "No usage data available")
                        .enabled(false)
                        .build(app)?;
                    menu_builder = menu_builder.item(&no_data);
                } else {
                    // Add model breakdowns
                    for breakdown in &today.model_breakdowns {
                        let model_name = format_model_name(&breakdown.model_name);
                        let cost_str = format!("{}: ${:.2}", model_name, breakdown.cost);
                        let model_item = MenuItemBuilder::with_id(
                            &format!("model_{}", breakdown.model_name),
                            &cost_str,
                        )
                        .enabled(false)
                        .build(app)?;
                        menu_builder = menu_builder.item(&model_item);
                    }
                }
            } else {
                let no_data = MenuItemBuilder::with_id("no_data", "No usage data for today")
                    .enabled(false)
                    .build(app)?;
                menu_builder = menu_builder.item(&no_data);
            }
        }
        None => {
            // No data available - show install link
            let install_link = MenuItemBuilder::with_id("install_ccusage", "Install ccusage CLI")
                .build(app)?;
            menu_builder = menu_builder.item(&install_link);
        }
    }

    menu_builder = menu_builder.separator();

    let refresh = MenuItemBuilder::with_id("refresh", "Refresh")
        .build(app)?;
    menu_builder = menu_builder.item(&refresh).separator();

    // Check if autostart is enabled to set initial state
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    
    let launch_on_startup = CheckMenuItemBuilder::with_id("launch_on_startup", "Launch on startup")
        .checked(autostart_enabled)
        .build(app)?;
    
    menu_builder = menu_builder.item(&launch_on_startup).separator();
    
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .accelerator("Cmd+Q")
        .build(app)?;
    
    menu_builder = menu_builder.item(&quit);

    Ok(menu_builder.build()?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            toggle_autostart, 
            is_autostart_enabled, 
            fetch_usage_data,
            get_cached_usage_data
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_handle = app.handle().clone();
            
            // Build initial menu
            tauri::async_runtime::spawn(async move {
                match build_menu_with_usage_data(&app_handle).await {
                    Ok(menu) => {
                        let tray = TrayIconBuilder::new()
                            .icon(
                                tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                                    .unwrap()
                                    .to_owned(),
                            )
                            .icon_as_template(true)
                            .menu(&menu)
                            .show_menu_on_left_click(true)
                            .on_menu_event({
                                let app_handle = app_handle.clone();
                                move |app, event| match event.id().as_ref() {
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
                                    "install_ccusage" => {
                                        let _ = tauri_plugin_opener::open_url(
                                            "https://github.com/ryoppippi/ccusage",
                                            None::<String>,
                                        );
                                    }
                                    "refresh" => {
                                        let app_handle = app_handle.clone();
                                        tauri::async_runtime::spawn(async move {
                                            // Rebuild menu with fresh data
                                            match build_menu_with_usage_data(&app_handle).await {
                                                Ok(new_menu) => {
                                                    if let Some(tray) = app_handle.tray_by_id("main") {
                                                        let _ = tray.set_menu(Some(new_menu));
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!("Failed to refresh menu: {}", e);
                                                }
                                            }
                                        });
                                    }
                                    _ => {}
                                }
                            })
                            .build(&app_handle)
                            .unwrap();

                        // Store tray reference
                        let _ = tray;
                    }
                    Err(e) => {
                        eprintln!("Failed to build initial menu: {}", e);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}