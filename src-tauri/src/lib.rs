use tauri::{
    tray::{TrayIconBuilder},
    Manager, WebviewUrl, WebviewWindowBuilder,
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

// Model name formatting is now handled in the React component

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
            println!("🚀 Starting app setup...");
            
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_handle = app.handle().clone();
            
            // Create hidden popup window at startup
            println!("🪟 Creating popup window...");
            let popup_window = WebviewWindowBuilder::new(
                app,
                "popup",
                WebviewUrl::App("/".into())
            )
            .title("CCUsage Debug")
            .inner_size(300.0, 400.0)
            .resizable(true)  // Temporarily resizable for debugging
            .decorations(true)  // Temporarily add decorations for debugging
            .always_on_top(true)
            .skip_taskbar(false)  // Show in taskbar temporarily for debugging
            .transparent(false)  // Remove transparency for debugging
            .shadow(true)
            .visible(false)
            .build();
            
            match popup_window {
                Ok(window) => {
                    println!("✅ Popup window created successfully: {}", window.label());
                }
                Err(e) => {
                    println!("❌ Failed to create popup window: {}", e);
                    return Err(e.into());
                }
            }

            // Create tray icon
            println!("🔘 Creating tray icon...");
            let tray_result = TrayIconBuilder::with_id("main")
                .icon(
                    tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                        .unwrap()
                        .to_owned(),
                )
                .icon_as_template(true)
                .on_tray_icon_event({
                    let app_handle = app_handle.clone();
                    move |_tray, event| {
                        println!("🖱️ Tray icon event received: {:?}", event);
                        
                        match event {
                            tauri::tray::TrayIconEvent::Click { position, rect, button_state, .. } => {
                                println!("👆 Click event detected at position: {:?}, button_state: {:?}", position, button_state);
                                println!("📍 Tray rect: {:?}", rect);
                                
                                // Only respond to button release (Up), not button press (Down)
                                if button_state != tauri::tray::MouseButtonState::Up {
                                    println!("🚫 Ignoring button press, waiting for release...");
                                    return;
                                }
                                
                                // Toggle popup window
                                if let Some(window) = app_handle.get_webview_window("popup") {
                                    println!("🪟 Found popup window");
                                    let is_visible = window.is_visible().unwrap_or(false);
                                    println!("👁️ Window visible: {}", is_visible);
                                    
                                    if is_visible {
                                        println!("🙈 Hiding window...");
                                        match window.hide() {
                                            Ok(_) => println!("✅ Window hidden successfully"),
                                            Err(e) => println!("❌ Failed to hide window: {}", e),
                                        }
                                    } else {
                                        println!("👀 Showing window...");
                                        
                                        // Position window near tray icon
                                        // Extract coordinates from Position and Size enums
                                        let (tray_x, tray_y) = match rect.position {
                                            tauri::Position::Physical(pos) => (pos.x as f64, pos.y as f64),
                                            tauri::Position::Logical(pos) => (pos.x, pos.y),
                                        };
                                        let (tray_width, tray_height) = match rect.size {
                                            tauri::Size::Physical(size) => (size.width as f64, size.height as f64),
                                            tauri::Size::Logical(size) => (size.width, size.height),
                                        };
                                        
                                        // Center window horizontally under tray icon
                                        let window_x = tray_x + (tray_width / 2.0) - 150.0; // 150 = half window width
                                        let window_y = tray_y + tray_height + 8.0; // 8px gap below tray
                                        
                                        println!("📍 Positioning window at ({}, {})", window_x, window_y);
                                        
                                        match window.set_position(tauri::PhysicalPosition::new(window_x as i32, window_y as i32)) {
                                            Ok(_) => println!("✅ Window positioned successfully"),
                                            Err(e) => println!("❌ Failed to position window: {}", e),
                                        }
                                        
                                        match window.show() {
                                            Ok(_) => {
                                                println!("✅ Window shown successfully");
                                                match window.set_focus() {
                                                    Ok(_) => println!("✅ Window focused successfully"),
                                                    Err(e) => println!("❌ Failed to focus window: {}", e),
                                                }
                                            }
                                            Err(e) => println!("❌ Failed to show window: {}", e),
                                        }
                                    }
                                } else {
                                    println!("❌ Popup window not found! Attempting to create it...");
                                    
                                    // Try to create window dynamically
                                    let window_result = WebviewWindowBuilder::new(
                                        &app_handle,
                                        "popup",
                                        WebviewUrl::App("/".into())
                                    )
                                    .title("CCUsage")
                                    .inner_size(300.0, 400.0)
                                    .resizable(false)
                                    .decorations(false)
                                    .always_on_top(true)
                                    .skip_taskbar(true)
                                    .transparent(true)
                                    .shadow(true)
                                    .visible(true)  // Show immediately when created
                                    .build();
                                    
                                    match window_result {
                                        Ok(window) => {
                                            println!("✅ Dynamic window created and shown");
                                            let _ = window.set_focus();
                                        }
                                        Err(e) => {
                                            println!("❌ Failed to create dynamic window: {}", e);
                                        }
                                    }
                                }
                            }
                            tauri::tray::TrayIconEvent::DoubleClick { .. } => {
                                println!("👆👆 Double click event detected!");
                            }
                            _ => {
                                println!("🤷 Other tray event: {:?}", event);
                            }
                        }
                    }
                })
                .build(&app_handle);
                
            match tray_result {
                Ok(_) => println!("✅ Tray icon created successfully"),
                Err(e) => {
                    println!("❌ Failed to create tray icon: {}", e);
                    return Err(e.into());
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}