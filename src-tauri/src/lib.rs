use tauri::{
    tray::{TrayIconBuilder},
    Manager,
};
use tauri_plugin_autostart::ManagerExt;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageData {
    today_data: Option<Vec<ModelBreakdown>>,
    five_hr_data: Option<Vec<ModelBreakdown>>,
    one_hr_data: Option<Vec<ModelBreakdown>>,
    week_data: Option<Vec<ModelBreakdown>>,
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

#[tauri::command]
async fn fetch_all_usage_data() -> Result<UsageData, String> {
    // Fetch all data concurrently
    let (today, five_hr, one_hr, week) = tokio::join!(
        fetch_today_data(),
        fetch_blocks_data("5"),
        fetch_blocks_data("1"),
        fetch_week_data()
    );

    Ok(UsageData {
        today_data: today,
        five_hr_data: five_hr,
        one_hr_data: one_hr,
        week_data: week,
    })
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
            fetch_all_usage_data,
            toggle_autostart, 
            is_autostart_enabled
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_handle = app.handle().clone();
            
            // Create tray icon
            let _tray = TrayIconBuilder::with_id("main")
                .icon(
                    tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                        .unwrap()
                        .to_owned(),
                )
                .icon_as_template(true)
                .on_tray_icon_event({
                    let app_handle = app_handle.clone();
                    move |_tray, event| {
                        if let tauri::tray::TrayIconEvent::Click { .. } = event {
                            // Toggle popup window
                            if let Some(window) = app_handle.get_webview_window("popup") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(&app_handle)
                .unwrap();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}