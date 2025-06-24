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
struct TokenCounts {
    #[serde(rename = "inputTokens")]
    input_tokens: u64,
    #[serde(rename = "outputTokens")]
    output_tokens: u64,
    #[serde(rename = "cacheCreationInputTokens")]
    cache_creation_tokens: u64,
    #[serde(rename = "cacheReadInputTokens")]
    cache_read_tokens: u64,
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
    #[serde(rename = "tokenCounts")]
    token_counts: TokenCounts,
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

#[derive(Debug, Clone)]
struct AllPeriodsData {
    today_data: Option<Vec<ModelBreakdown>>,
    five_hr_data: Option<Vec<ModelBreakdown>>,
    one_hr_data: Option<Vec<ModelBreakdown>>,
    week_data: Option<Vec<ModelBreakdown>>,
    last_updated: Option<Instant>,
}

static ALL_DATA_CACHE: Mutex<AllPeriodsData> = Mutex::new(AllPeriodsData {
    today_data: None,
    five_hr_data: None,
    one_hr_data: None,
    week_data: None,
    last_updated: None,
});

static IS_REFRESHING: AtomicBool = AtomicBool::new(false);

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

async fn fetch_blocks_data(session_length: &str) -> Option<Vec<ModelBreakdown>> {
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
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: CcusageBlocksResponse = serde_json::from_str(&stdout).ok()?;
    
    if let Some(latest_block) = response.blocks.first() {
        if !latest_block.model_breakdowns.is_empty() {
            Some(latest_block.model_breakdowns.clone())
        } else {
            // Create breakdown based on models present and token counts
            let cost_per_model = latest_block.cost_usd / latest_block.models.len() as f64;
            let tokens_per_model = latest_block.token_counts.input_tokens / latest_block.models.len() as u64;
            let output_tokens_per_model = latest_block.token_counts.output_tokens / latest_block.models.len() as u64;
            
            Some(latest_block.models.iter().map(|model| ModelBreakdown {
                model_name: model.clone(),
                input_tokens: tokens_per_model,
                output_tokens: output_tokens_per_model,
                cache_creation_tokens: latest_block.token_counts.cache_creation_tokens / latest_block.models.len() as u64,
                cache_read_tokens: latest_block.token_counts.cache_read_tokens / latest_block.models.len() as u64,
                cost: cost_per_model,
            }).collect())
        }
    } else {
        None
    }
}

async fn fetch_week_data() -> Option<Vec<ModelBreakdown>> {
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
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: CcusageResponse = serde_json::from_str(&stdout).ok()?;
    
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
    
    Some(aggregated.into_values().collect())
}

async fn refresh_all_data() {
    // Set refresh flag
    IS_REFRESHING.store(true, Ordering::Relaxed);
    
    // Fetch all data concurrently
    let (today, five_hr, one_hr, week) = tokio::join!(
        fetch_today_data(),
        fetch_blocks_data("5"),
        fetch_blocks_data("1"),
        fetch_week_data()
    );

    // Update cache
    {
        let mut cache = ALL_DATA_CACHE.lock().unwrap();
        cache.today_data = today;
        cache.five_hr_data = five_hr;
        cache.one_hr_data = one_hr;
        cache.week_data = week;
        cache.last_updated = Some(Instant::now());
    }
    
    // Add small delay to let UI stabilize
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    
    // Clear refresh flag
    IS_REFRESHING.store(false, Ordering::Relaxed);
}

async fn build_menu_with_all_periods(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    // Check if we need to refresh (no data or data is older than 5 minutes)
    // But skip if a manual refresh is already in progress
    let should_refresh = {
        if IS_REFRESHING.load(Ordering::Relaxed) {
            false // Don't refresh if already refreshing
        } else {
            let cache = ALL_DATA_CACHE.lock().unwrap();
            match cache.last_updated {
                Some(last_update) => last_update.elapsed().as_secs() > 300, // 5 minutes
                None => true, // No data yet
            }
        }
    };

    if should_refresh {
        refresh_all_data().await;
    }

    let mut menu_builder = MenuBuilder::new(app);

    // Get data from cache
    let cache_data = {
        let cache = ALL_DATA_CACHE.lock().unwrap();
        (
            cache.today_data.clone(),
            cache.five_hr_data.clone(),
            cache.one_hr_data.clone(),
            cache.week_data.clone(),
        )
    };

    let (today_data, five_hr_data, one_hr_data, week_data) = cache_data;

    // 1 Hour section (first)
    let one_hr_title = MenuItemBuilder::with_id("1hr_title", "1 Hr")
        .enabled(false)
        .build(app)?;
    menu_builder = menu_builder.item(&one_hr_title);

    if let Some(breakdowns) = one_hr_data {
        for breakdown in &breakdowns {
            let model_name = format_model_name(&breakdown.model_name);
            let input_k = breakdown.input_tokens as f64 / 1000.0;
            let output_k = breakdown.output_tokens as f64 / 1000.0;
            let cost_str = format!("{}: ${:.2} (In: {:.1}K, Out: {:.1}K)", 
                model_name, breakdown.cost, input_k, output_k);
            let model_item = MenuItemBuilder::with_id(
                &format!("1hr_{}", breakdown.model_name),
                &cost_str,
            )
            .build(app)?;
            menu_builder = menu_builder.item(&model_item);
        }
    } else {
        let no_data = MenuItemBuilder::with_id("1hr_no_data", "No data available")
            .build(app)?;
        menu_builder = menu_builder.item(&no_data);
    }

    menu_builder = menu_builder.separator();

    // 5 Hour section (second)
    let five_hr_title = MenuItemBuilder::with_id("5hr_title", "5 Hr")
        .enabled(false)
        .build(app)?;
    menu_builder = menu_builder.item(&five_hr_title);

    if let Some(breakdowns) = five_hr_data {
        for breakdown in &breakdowns {
            let model_name = format_model_name(&breakdown.model_name);
            let input_k = breakdown.input_tokens as f64 / 1000.0;
            let output_k = breakdown.output_tokens as f64 / 1000.0;
            let cost_str = format!("{}: ${:.2} (In: {:.1}K, Out: {:.1}K)", 
                model_name, breakdown.cost, input_k, output_k);
            let model_item = MenuItemBuilder::with_id(
                &format!("5hr_{}", breakdown.model_name),
                &cost_str,
            )
            .build(app)?;
            menu_builder = menu_builder.item(&model_item);
        }
    } else {
        let no_data = MenuItemBuilder::with_id("5hr_no_data", "No data available")
            .build(app)?;
        menu_builder = menu_builder.item(&no_data);
    }

    menu_builder = menu_builder.separator();

    // Today section (third)
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

    // Week section (fourth)
    let week_title = MenuItemBuilder::with_id("week_title", "Week")
        .enabled(false)
        .build(app)?;
    menu_builder = menu_builder.item(&week_title);

    if let Some(breakdowns) = week_data {
        for breakdown in &breakdowns {
            let model_name = format_model_name(&breakdown.model_name);
            let input_k = breakdown.input_tokens as f64 / 1000.0;
            let output_k = breakdown.output_tokens as f64 / 1000.0;
            let cost_str = format!("{}: ${:.2} (In: {:.1}K, Out: {:.1}K)", 
                model_name, breakdown.cost, input_k, output_k);
            let model_item = MenuItemBuilder::with_id(
                &format!("week_{}", breakdown.model_name),
                &cost_str,
            )
            .build(app)?;
            menu_builder = menu_builder.item(&model_item);
        }
    } else {
        // Check if ccusage is available
        let install_link = MenuItemBuilder::with_id("install_ccusage", "Install ccusage CLI")
            .build(app)?;
        menu_builder = menu_builder.item(&install_link);
    }

    menu_builder = menu_builder.separator();

    // Refresh button
    let refresh = MenuItemBuilder::with_id("refresh", "Refresh")
        .build(app)?;
    menu_builder = menu_builder.item(&refresh).separator();

    // Launch on startup
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let launch_on_startup = CheckMenuItemBuilder::with_id("launch_on_startup", "Launch on startup")
        .checked(autostart_enabled)
        .build(app)?;
    menu_builder = menu_builder.item(&launch_on_startup).separator();

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
            
            // Build initial menu
            tauri::async_runtime::spawn(async move {
                match build_menu_with_all_periods(&app_handle).await {
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
                                        tauri::async_runtime::spawn(async move {
                                            // Force refresh all data
                                            refresh_all_data().await;
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