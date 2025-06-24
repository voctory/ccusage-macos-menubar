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
struct SessionData {
    breakdowns: Option<Vec<ModelBreakdown>>,
    total_cost: f64,
    session_start: Option<String>,
    session_end: Option<String>,
    last_updated: Option<Instant>,
}

static SESSION_CACHE: Mutex<SessionData> = Mutex::new(SessionData {
    breakdowns: None,
    total_cost: 0.0,
    session_start: None,
    session_end: None,
    last_updated: None,
});

// Removed AppSettings as we now always show cost

static IS_REFRESHING: AtomicBool = AtomicBool::new(false);

// Removed settings functions as we now always show cost

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

async fn fetch_session_data() -> Option<(Vec<ModelBreakdown>, String, String)> {
    let output = Command::new("npx")
        .args(&[
            "ccusage@latest", 
            "blocks", 
            "--json", 
            "--breakdown", 
            "--recent",
            "--session-length",
            "5"  // 5 hour rolling session
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    
    // Aggregate all blocks in the 5-hour session
    if let Some(blocks) = data["blocks"].as_array() {
        if blocks.is_empty() {
            return None;
        }
        
        // Get session start and end times from first and last blocks
        let first_block = &blocks[0];
        let last_block = &blocks[blocks.len() - 1];
        
        let session_start = first_block["startTime"].as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Local).format("%I:%M %p").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
            
        let session_end = last_block["endTime"].as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Local).format("%I:%M %p").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        
        let mut aggregated: std::collections::HashMap<String, ModelBreakdown> = std::collections::HashMap::new();
        
        for block in blocks {
            // First try modelBreakdowns
            if let Some(breakdowns) = block["modelBreakdowns"].as_array() {
                for breakdown in breakdowns {
                    let model_name = breakdown["modelName"].as_str().unwrap_or_default().to_string();
                    let entry = aggregated.entry(model_name.clone()).or_insert(ModelBreakdown {
                        model_name,
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_creation_tokens: 0,
                        cache_read_tokens: 0,
                        cost: 0.0,
                    });
                    entry.input_tokens += breakdown["inputTokens"].as_u64().unwrap_or(0);
                    entry.output_tokens += breakdown["outputTokens"].as_u64().unwrap_or(0);
                    entry.cache_creation_tokens += breakdown["cacheCreationTokens"].as_u64().unwrap_or(0);
                    entry.cache_read_tokens += breakdown["cacheReadTokens"].as_u64().unwrap_or(0);
                    entry.cost += breakdown["cost"].as_f64().unwrap_or(0.0);
                }
            } else if let Some(models) = block["models"].as_array() {
                // Fallback: distribute cost and tokens evenly among models
                let cost_usd = block["costUSD"].as_f64().unwrap_or(0.0);
                let cost_per_model = cost_usd / models.len() as f64;
                
                let token_counts = &block["tokenCounts"];
                let input_tokens = token_counts["inputTokens"].as_u64().unwrap_or(0) / models.len() as u64;
                let output_tokens = token_counts["outputTokens"].as_u64().unwrap_or(0) / models.len() as u64;
                let cache_creation = token_counts["cacheCreationInputTokens"].as_u64().unwrap_or(0) / models.len() as u64;
                let cache_read = token_counts["cacheReadInputTokens"].as_u64().unwrap_or(0) / models.len() as u64;
                
                for model in models {
                    let model_name = model.as_str().unwrap_or_default().to_string();
                    let entry = aggregated.entry(model_name.clone()).or_insert(ModelBreakdown {
                        model_name,
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_creation_tokens: 0,
                        cache_read_tokens: 0,
                        cost: 0.0,
                    });
                    entry.input_tokens += input_tokens;
                    entry.output_tokens += output_tokens;
                    entry.cache_creation_tokens += cache_creation;
                    entry.cache_read_tokens += cache_read;
                    entry.cost += cost_per_model;
                }
            }
        }
        
        Some((aggregated.into_values().collect(), session_start, session_end))
    } else {
        None
    }
}

// Removed fetch_blocks_data and fetch_week_data functions as they are no longer needed

async fn refresh_session_data(app_handle: &tauri::AppHandle) {
    // Set refresh flag
    IS_REFRESHING.store(true, Ordering::Relaxed);
    
    // Fetch 5-hour session data
    let session_data = fetch_session_data().await;
    
    // Calculate total cost and extract session times
    let (total_cost, session_start, session_end) = if let Some((ref breakdowns, ref start, ref end)) = session_data {
        let cost: f64 = breakdowns.iter().map(|b| b.cost).sum();
        (cost, Some(start.clone()), Some(end.clone()))
    } else {
        (0.0, None, None)
    };

    // Update cache
    {
        let mut cache = SESSION_CACHE.lock().unwrap();
        cache.breakdowns = session_data.map(|(breakdowns, _, _)| breakdowns);
        cache.total_cost = total_cost;
        cache.session_start = session_start;
        cache.session_end = session_end;
        cache.last_updated = Some(Instant::now());
    }
    
    // Always update tray title with cost
    if let Some(tray) = app_handle.tray_by_id("main") {
        let title = if total_cost > 0.0 {
            format!("${:.2}", total_cost)
        } else {
            String::new()
        };
        let _ = tray.set_title(Some(title));
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
    let (session_data, session_start, session_end) = {
        let cache = SESSION_CACHE.lock().unwrap();
        (cache.breakdowns.clone(), cache.session_start.clone(), cache.session_end.clone())
    };

    // 5 Hr Session section
    let session_title = MenuItemBuilder::with_id("session_title", "5 Hr Session")
        .enabled(false)
        .build(app)?;
    menu_builder = menu_builder.item(&session_title);

    if let Some(breakdowns) = session_data {
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

    // Session times
    if let (Some(start), Some(end)) = (session_start, session_end) {
        let session_start_item = MenuItemBuilder::with_id("session_start", &format!("Session start: {}", start))
            .enabled(false)
            .build(app)?;
        let session_end_item = MenuItemBuilder::with_id("session_end", &format!("Session end: {}", end))
            .enabled(false)
            .build(app)?;
        menu_builder = menu_builder.item(&session_start_item).item(&session_end_item).separator();
    }

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
                            let cache = SESSION_CACHE.lock().unwrap();
                            cache.last_updated.is_some() // Only auto-refresh if we've refreshed at least once
                        };
                        if should_refresh {
                            refresh_session_data(&periodic_handle).await;
                        }
                    }
                }
            });

            tauri::async_runtime::spawn(async move {
                // Initial data refresh on app startup
                refresh_session_data(&app_handle).await;
                
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
                                            refresh_session_data(&app_handle).await;
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