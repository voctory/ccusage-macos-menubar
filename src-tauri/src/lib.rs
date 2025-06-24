use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, CheckMenuItemBuilder, SubmenuBuilder},
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
struct BlockUsage {
    id: String,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "endTime")]
    end_time: String,
    #[serde(rename = "isActive")]
    is_active: bool,
    #[serde(rename = "costUSD")]
    cost_usd: f64,
    models: Vec<String>,
    #[serde(rename = "modelBreakdowns", default)]
    model_breakdowns: Vec<ModelBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CcusageResponse {
    daily: Vec<DailyUsage>,
    totals: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CcusageBlocksResponse {
    blocks: Vec<BlockUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TimePeriod {
    Today,
    FiveHours,
    OneHour,
    Week,
}

impl TimePeriod {
    fn display_name(&self) -> &'static str {
        match self {
            TimePeriod::Today => "Today",
            TimePeriod::FiveHours => "5 Hrs",
            TimePeriod::OneHour => "1 Hr",
            TimePeriod::Week => "Week",
        }
    }

    fn menu_id(&self) -> &'static str {
        match self {
            TimePeriod::Today => "period_today",
            TimePeriod::FiveHours => "period_5hrs",
            TimePeriod::OneHour => "period_1hr",
            TimePeriod::Week => "period_week",
        }
    }
}

#[derive(Debug, Clone)]
struct UsageCache {
    daily_data: Option<CcusageResponse>,
    blocks_data: Option<CcusageBlocksResponse>,
    current_period: TimePeriod,
    last_updated: Option<Instant>,
    last_error: Option<String>,
}

static USAGE_CACHE: Mutex<UsageCache> = Mutex::new(UsageCache {
    daily_data: None,
    blocks_data: None,
    current_period: TimePeriod::Today,
    last_updated: None,
    last_error: None,
});

#[tauri::command]
async fn fetch_usage_data(period: String) -> Result<String, String> {
    let period_enum = match period.as_str() {
        "today" => TimePeriod::Today,
        "5hrs" => TimePeriod::FiveHours,
        "1hr" => TimePeriod::OneHour,
        "week" => TimePeriod::Week,
        _ => TimePeriod::Today,
    };

    match period_enum {
        TimePeriod::Today => {
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
                cache.daily_data = Some(response.clone());
                cache.current_period = period_enum;
                cache.last_updated = Some(Instant::now());
                cache.last_error = None;
            }

            Ok(serde_json::to_string(&response).unwrap())
        }
        TimePeriod::FiveHours | TimePeriod::OneHour => {
            let session_length = match period_enum {
                TimePeriod::FiveHours => "5",
                TimePeriod::OneHour => "1",
                _ => "5",
            };

            let output = Command::new("npx")
                .args(&[
                    "ccusage@latest", 
                    "blocks", 
                    "--json", 
                    "--breakdown", 
                    "--recent",
                    "--session-length",
                    session_length
                ])
                .output()
                .await
                .map_err(|e| format!("Failed to run ccusage: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("ccusage command failed: {}", stderr));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let response: CcusageBlocksResponse = serde_json::from_str(&stdout)
                .map_err(|e| format!("Failed to parse ccusage output: {}", e))?;

            // Update cache
            {
                let mut cache = USAGE_CACHE.lock().unwrap();
                cache.blocks_data = Some(response.clone());
                cache.current_period = period_enum;
                cache.last_updated = Some(Instant::now());
                cache.last_error = None;
            }

            Ok(serde_json::to_string(&response).unwrap())
        }
        TimePeriod::Week => {
            // Calculate date 7 days ago
            let now = chrono::Utc::now();
            let week_ago = now - chrono::Duration::days(7);
            let since_date = week_ago.format("%Y%m%d").to_string();

            let output = Command::new("npx")
                .args(&[
                    "ccusage@latest", 
                    "daily", 
                    "--json", 
                    "--breakdown",
                    "--since",
                    &since_date
                ])
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
                cache.daily_data = Some(response.clone());
                cache.current_period = period_enum;
                cache.last_updated = Some(Instant::now());
                cache.last_error = None;
            }

            Ok(serde_json::to_string(&response).unwrap())
        }
    }
}

#[tauri::command]
async fn get_cached_usage_data() -> Result<Option<String>, String> {
    let cache = USAGE_CACHE.lock().unwrap();
    match cache.current_period {
        TimePeriod::Today | TimePeriod::Week => {
            if let Some(ref data) = cache.daily_data {
                Ok(Some(serde_json::to_string(data).unwrap()))
            } else {
                Ok(None)
            }
        }
        TimePeriod::FiveHours | TimePeriod::OneHour => {
            if let Some(ref data) = cache.blocks_data {
                Ok(Some(serde_json::to_string(data).unwrap()))
            } else {
                Ok(None)
            }
        }
    }
}

#[tauri::command]
async fn get_current_period() -> String {
    let cache = USAGE_CACHE.lock().unwrap();
    cache.current_period.display_name().to_string()
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

fn extract_model_breakdowns_from_data(data_str: &str, period: TimePeriod) -> Vec<ModelBreakdown> {
    match period {
        TimePeriod::Today | TimePeriod::Week => {
            if let Ok(response) = serde_json::from_str::<CcusageResponse>(data_str) {
                if period == TimePeriod::Week {
                    // Aggregate all daily data for the week
                    let mut aggregated: std::collections::HashMap<String, ModelBreakdown> = std::collections::HashMap::new();
                    
                    for day in &response.daily {
                        for breakdown in &day.model_breakdowns {
                            let entry = aggregated.entry(breakdown.model_name.clone()).or_insert(ModelBreakdown {
                                model_name: breakdown.model_name.clone(),
                                input_tokens: 0,
                                output_tokens: 0,
                                cache_creation_tokens: 0,
                                cache_read_tokens: 0,
                                cost: 0.0,
                            });
                            entry.input_tokens += breakdown.input_tokens;
                            entry.output_tokens += breakdown.output_tokens;
                            entry.cache_creation_tokens += breakdown.cache_creation_tokens;
                            entry.cache_read_tokens += breakdown.cache_read_tokens;
                            entry.cost += breakdown.cost;
                        }
                    }
                    
                    aggregated.into_values().collect()
                } else if let Some(today) = response.daily.first() {
                    today.model_breakdowns.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        }
        TimePeriod::FiveHours | TimePeriod::OneHour => {
            if let Ok(response) = serde_json::from_str::<CcusageBlocksResponse>(data_str) {
                if let Some(latest_block) = response.blocks.first() {
                    // If block has breakdown, use it; otherwise estimate from models and total cost
                    if !latest_block.model_breakdowns.is_empty() {
                        latest_block.model_breakdowns.clone()
                    } else {
                        // Estimate breakdown based on models present
                        let cost_per_model = latest_block.cost_usd / latest_block.models.len() as f64;
                        latest_block.models.iter().map(|model| ModelBreakdown {
                            model_name: model.clone(),
                            input_tokens: 0,
                            output_tokens: 0,
                            cache_creation_tokens: 0,
                            cache_read_tokens: 0,
                            cost: cost_per_model,
                        }).collect()
                    }
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        }
    }
}

async fn build_menu_with_usage_data(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    // Get current period from cache
    let current_period = {
        let cache = USAGE_CACHE.lock().unwrap();
        cache.current_period
    };

    // Try to fetch fresh data first, fall back to cache
    let usage_result = fetch_usage_data(match current_period {
        TimePeriod::Today => "today",
        TimePeriod::FiveHours => "5hrs",
        TimePeriod::OneHour => "1hr",
        TimePeriod::Week => "week",
    }.to_string()).await;
    
    let usage_data = match usage_result {
        Ok(data) => Some(data),
        Err(_) => {
            // Try to get cached data
            get_cached_usage_data().await.unwrap_or(None)
        }
    };

    // Create period submenu
    let today_item = MenuItemBuilder::with_id("period_today", "Today").build(app)?;
    let five_hrs_item = MenuItemBuilder::with_id("period_5hrs", "5 Hrs").build(app)?;
    let one_hr_item = MenuItemBuilder::with_id("period_1hr", "1 Hr").build(app)?;
    let week_item = MenuItemBuilder::with_id("period_week", "Week").build(app)?;

    let period_submenu = SubmenuBuilder::new(app, format!("CCUsage - {}", current_period.display_name()))
        .item(&today_item)
        .item(&five_hrs_item)
        .item(&one_hr_item)
        .item(&week_item)
        .build()?;

    let mut menu_builder = MenuBuilder::new(app).item(&period_submenu).separator();

    match usage_data {
        Some(data) => {
            let model_breakdowns = extract_model_breakdowns_from_data(&data, current_period);
            
            if model_breakdowns.is_empty() {
                let no_data = MenuItemBuilder::with_id("no_data", "No usage data available")
                    .build(app)?;
                menu_builder = menu_builder.item(&no_data);
            } else {
                // Add model breakdowns - make them clickable/readable (not disabled)
                for breakdown in &model_breakdowns {
                    let model_name = format_model_name(&breakdown.model_name);
                    let cost_str = format!("{}: ${:.2}", model_name, breakdown.cost);
                    let model_item = MenuItemBuilder::with_id(
                        &format!("model_{}", breakdown.model_name),
                        &cost_str,
                    )
                    .build(app)?; // Removed .enabled(false) to make them readable
                    menu_builder = menu_builder.item(&model_item);
                }
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
            get_cached_usage_data,
            get_current_period
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
                                    "refresh" | "period_today" | "period_5hrs" | "period_1hr" | "period_week" => {
                                        let app_handle = app_handle.clone();
                                        let period = match event.id().as_ref() {
                                            "period_today" => "today",
                                            "period_5hrs" => "5hrs", 
                                            "period_1hr" => "1hr",
                                            "period_week" => "week",
                                            _ => "today", // refresh uses current period
                                        };
                                        
                                        tauri::async_runtime::spawn(async move {
                                            // Update period if different
                                            if event.id().as_ref().starts_with("period_") {
                                                let mut cache = USAGE_CACHE.lock().unwrap();
                                                cache.current_period = match period {
                                                    "today" => TimePeriod::Today,
                                                    "5hrs" => TimePeriod::FiveHours,
                                                    "1hr" => TimePeriod::OneHour,
                                                    "week" => TimePeriod::Week,
                                                    _ => TimePeriod::Today,
                                                };
                                            }
                                            
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