use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, CheckMenuItemBuilder},
    tray::{TrayIconBuilder},
    Manager,
};
use tauri_plugin_autostart::ManagerExt;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, atomic::{AtomicBool, Ordering}};
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
struct TodayData {
    breakdowns: Option<Vec<ModelBreakdown>>,
    total_cost: f64,
    last_updated: Option<Instant>,
}

static TODAY_CACHE: Mutex<TodayData> = Mutex::new(TodayData {
    breakdowns: None,
    total_cost: 0.0,
    last_updated: None,
});

// Settings for persisting user preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettings {
    show_cost_in_menubar: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_cost_in_menubar: true,
        }
    }
}

static IS_REFRESHING: AtomicBool = AtomicBool::new(false);

fn get_settings_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ccusage-menubar")
        .join("settings.json")
}

fn load_settings() -> AppSettings {
    let path = get_settings_path();
    if path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str(&contents) {
                return settings;
            }
        }
    }
    AppSettings::default()
}

fn save_settings(settings: &AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, contents)?;
    Ok(())
}

fn format_model_name(model_name: &str) -> String {
    match model_name {
        "claude-opus-4-20250514" => "Opus 4".to_string(),
        "claude-sonnet-4-20250514" => "Sonnet 4".to_string(),
        "claude-3-5-sonnet-20241022" => "Sonnet 3.5".to_string(),
        "claude-3-haiku-20240307" => "Haiku".to_string(),
        _ => {
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

async fn fetch_today_data() -> Option<Vec<ModelBreakdown>> {
    let output = Command::new("npx")
        .args(&["ccusage@latest", "daily", "--json", "--breakdown"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: CcusageResponse = serde_json::from_str(&stdout).ok()?;
    
    response.daily.first().map(|day| day.model_breakdowns.clone())
}

// Removed fetch_blocks_data and fetch_week_data functions as they are no longer needed

async fn refresh_today_data(app_handle: &tauri::AppHandle) {
    // Set refresh flag
    IS_REFRESHING.store(true, Ordering::Relaxed);
    
    // Fetch today's data
    let today_data = fetch_today_data().await;
    
    // Calculate total cost
    let total_cost = if let Some(ref breakdowns) = today_data {
        breakdowns.iter().map(|b| b.cost).sum()
    } else {
        0.0
    };

    // Update cache
    {
        let mut cache = TODAY_CACHE.lock().unwrap();
        cache.breakdowns = today_data;
        cache.total_cost = total_cost;
        cache.last_updated = Some(Instant::now());
    }
    
    // Update tray title if enabled
    let settings = load_settings();
    if settings.show_cost_in_menubar {
        if let Some(tray) = app_handle.tray_by_id("main") {
            let title = if total_cost > 0.0 {
                format!("${:.2}", total_cost)
            } else {
                String::new()
            };
            let _ = tray.set_title(Some(title));
        }
    } else {
        // Clear title if disabled
        if let Some(tray) = app_handle.tray_by_id("main") {
            let _ = tray.set_title(None::<String>);
        }
    }
    
    // Clear refresh flag
    IS_REFRESHING.store(false, Ordering::Relaxed);
}

async fn build_menu(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let mut menu_builder = MenuBuilder::new(app);

    // CCUsage header (simple, no timestamp)
    let ccusage_header = MenuItemBuilder::with_id("ccusage_header", "CCUsage")
        .build(app)?;
    menu_builder = menu_builder.item(&ccusage_header).separator();

    // Get data from cache
    let today_data = {
        let cache = TODAY_CACHE.lock().unwrap();
        cache.breakdowns.clone()
    };

    // Today section
    let today_title = MenuItemBuilder::with_id("today_title", "Today")
        .enabled(false)
        .build(app)?;
    menu_builder = menu_builder.item(&today_title);

    if let Some(breakdowns) = today_data {
        for breakdown in &breakdowns {
            let model_name = format_model_name(&breakdown.model_name);
            let input_k = breakdown.input_tokens as f64 / 1000.0;
            let output_k = breakdown.output_tokens as f64 / 1000.0;
            let cost_str = format!("{}: ${:.2} (In: {:.1}K, Out: {:.1}K)", 
                model_name, breakdown.cost, input_k, output_k);
            let model_item = MenuItemBuilder::with_id(
                &format!("today_{}", breakdown.model_name),
                &cost_str,
            )
            .build(app)?;
            menu_builder = menu_builder.item(&model_item);
        }
    } else {
        let no_data = MenuItemBuilder::with_id("today_no_data", "No data available")
            .build(app)?;
        menu_builder = menu_builder.item(&no_data);
    }

    menu_builder = menu_builder.separator();

    // Show cost in menubar toggle
    let settings = load_settings();
    let show_cost = CheckMenuItemBuilder::with_id("show_cost_in_menubar", "Show cost in menubar")
        .checked(settings.show_cost_in_menubar)
        .build(app)?;
    menu_builder = menu_builder.item(&show_cost);

    // Launch on startup
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let launch_on_startup = CheckMenuItemBuilder::with_id("launch_on_startup", "Launch on startup")
        .checked(autostart_enabled)
        .build(app)?;
    menu_builder = menu_builder.item(&launch_on_startup);

    // Refresh button
    let refresh = MenuItemBuilder::with_id("refresh", "Refresh")
        .build(app)?;
    menu_builder = menu_builder.item(&refresh).separator();

    // Quit
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .accelerator("Cmd+Q")
        .build(app)?;
    menu_builder = menu_builder.item(&quit);

    Ok(menu_builder.build()?)
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
            is_autostart_enabled
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_handle = app.handle().clone();
            
            // Start periodic refresh task
            let periodic_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(120)); // 2 minutes
                loop {
                    interval.tick().await;
                    // Only refresh if not already refreshing and we have initial data
                    if !IS_REFRESHING.load(Ordering::Relaxed) {
                        let should_refresh = {
                            let cache = TODAY_CACHE.lock().unwrap();
                            cache.last_updated.is_some() // Only auto-refresh if we've refreshed at least once
                        };
                        if should_refresh {
                            refresh_today_data(&periodic_handle).await;
                        }
                    }
                }
            });

            tauri::async_runtime::spawn(async move {
                // Initial data refresh on app startup
                refresh_today_data(&app_handle).await;
                
                match build_menu(&app_handle).await {
                    Ok(menu) => {
                        let tray = TrayIconBuilder::with_id("main")
                            .icon(
                                tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                                    .unwrap()
                                    .to_owned(),
                            )
                            .icon_as_template(true)
                            .menu(&menu)
                            .show_menu_on_left_click(true)
                            .on_menu_event({
                                let _app_handle = app_handle.clone();
                                move |app, event| match event.id().as_ref() {
                                    "ccusage_header" => {
                                        let _ = tauri_plugin_opener::open_url(
                                            "https://github.com/ryoppippi/ccusage",
                                            None::<String>,
                                        );
                                    }
                                    "quit" => {
                                        app.exit(0);
                                    }
                                    "show_cost_in_menubar" => {
                                        let app_handle = app.app_handle().clone();
                                        tauri::async_runtime::spawn(async move {
                                            // Toggle the setting
                                            let mut settings = load_settings();
                                            settings.show_cost_in_menubar = !settings.show_cost_in_menubar;
                                            let _ = save_settings(&settings);
                                            
                                            // Update tray title immediately
                                            if let Some(tray) = app_handle.tray_by_id("main") {
                                                if settings.show_cost_in_menubar {
                                                    // Show current cost
                                                    let cost = {
                                                        let cache = TODAY_CACHE.lock().unwrap();
                                                        cache.total_cost
                                                    };
                                                    let title = if cost > 0.0 {
                                                        format!("${:.2}", cost)
                                                    } else {
                                                        String::new()
                                                    };
                                                    let _ = tray.set_title(Some(title));
                                                } else {
                                                    // Hide cost
                                                    let _ = tray.set_title(None::<String>);
                                                }
                                            }
                                        });
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
                                    "refresh" => {
                                        let app_handle = app.app_handle().clone();
                                        tauri::async_runtime::spawn(async move {
                                            // Force refresh all data
                                            refresh_today_data(&app_handle).await;
                                            // Note: Menu will use fresh data on next open
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