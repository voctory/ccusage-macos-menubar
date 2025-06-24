use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{TrayIconBuilder},
    Manager,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Instant;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockData {
    id: String,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "endTime")]
    end_time: String,
    #[serde(rename = "isActive")]
    is_active: bool,
    #[serde(rename = "tokenCounts")]
    token_counts: TokenCounts,
    #[serde(rename = "costUSD")]
    cost_usd: f64,
    models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCounts {
    #[serde(rename = "inputTokens")]
    input_tokens: u64,
    #[serde(rename = "outputTokens")]
    output_tokens: u64,
    #[serde(rename = "cacheCreationInputTokens")]
    cache_creation_input_tokens: u64,
    #[serde(rename = "cacheReadInputTokens")]
    cache_read_input_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlocksResponse {
    blocks: Vec<BlockData>,
}


#[derive(Debug, Clone)]
struct SessionData {
    active_block: Option<BlockData>,
    last_updated: Option<Instant>,
}

static SESSION_CACHE: Mutex<SessionData> = Mutex::new(SessionData {
    active_block: None,
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

async fn fetch_session_data() -> Option<BlockData> {
    // Try multiple approaches to find and run ccusage
    let shell_commands = vec![
        // Use shell to ensure proper PATH resolution
        ("sh", vec!["-c", "npx ccusage@latest blocks --json --active"]),
        // Try with explicit PATH that includes common npm locations
        ("sh", vec!["-c", "PATH=/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:$HOME/.npm/bin:$HOME/.nvm/versions/node/*/bin:$HOME/.volta/bin:$PATH npx ccusage@latest blocks --json --active"]),
        // Try global ccusage if installed
        ("sh", vec!["-c", "ccusage blocks --json --active"]),
        // Try with explicit PATH for global ccusage
        ("sh", vec!["-c", "PATH=/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:$HOME/.npm/bin:$HOME/.nvm/versions/node/*/bin:$HOME/.volta/bin:$PATH ccusage blocks --json --active"]),
        // Try direct npx if in PATH
        ("npx", vec!["ccusage@latest", "blocks", "--json", "--active"]),
        // Try direct ccusage command
        ("ccusage", vec!["blocks", "--json", "--active"]),
    ];

    for (cmd, args) in shell_commands {
        let output = Command::new(cmd)
            .args(&args)
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                
                // Try to parse the response
                match serde_json::from_str::<BlocksResponse>(&stdout) {
                    Ok(response) => {
                        return response.blocks.into_iter().find(|block| block.is_active);
                    }
                    Err(e) => {
                        eprintln!("Failed to parse ccusage response: {}", e);
                        eprintln!("Response was: {}", stdout);
                        continue;
                    }
                }
            }
            Ok(output) => {
                eprintln!("ccusage command failed with status: {}", output.status);
                eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                continue;
            }
            Err(e) => {
                eprintln!("Failed to execute command '{}': {}", cmd, e);
                continue;
            }
        }
    }

    eprintln!("All attempts to fetch session data failed");
    None
}

// Removed fetch_blocks_data and fetch_week_data functions as they are no longer needed

async fn get_debug_info() -> String {
    let mut debug_info = String::new();
    
    // Get PATH environment variable
    debug_info.push_str("Environment:\n");
    if let Ok(path) = std::env::var("PATH") {
        debug_info.push_str(&format!("PATH: {}\n\n", path));
    } else {
        debug_info.push_str("PATH: (not set)\n\n");
    }
    
    // Test which command
    debug_info.push_str("Command availability:\n");
    
    let commands_to_test = vec![
        ("which npx", "npx location"),
        ("which node", "node location"),
        ("which ccusage", "ccusage location"),
        ("npx --version", "npx version"),
        ("node --version", "node version"),
        ("ccusage --version 2>&1 || echo 'not found'", "ccusage version"),
    ];
    
    for (cmd, desc) in commands_to_test {
        let output = Command::new("sh")
            .args(&["-c", cmd])
            .output()
            .await;
            
        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                debug_info.push_str(&format!("{}: {}\n", desc, stdout.trim()));
            }
            Ok(output) => {
                debug_info.push_str(&format!("{}: command failed ({})\n", desc, output.status));
            }
            Err(e) => {
                debug_info.push_str(&format!("{}: error - {}\n", desc, e));
            }
        }
    }
    
    // Test ccusage
    debug_info.push_str("\nTesting ccusage:\n");
    let ccusage_output = Command::new("sh")
        .args(&["-c", "npx ccusage@latest --version"])
        .output()
        .await;
        
    match ccusage_output {
        Ok(output) => {
            debug_info.push_str(&format!("Exit status: {}\n", output.status));
            debug_info.push_str(&format!("stdout: {}\n", String::from_utf8_lossy(&output.stdout)));
            debug_info.push_str(&format!("stderr: {}\n", String::from_utf8_lossy(&output.stderr)));
        }
        Err(e) => {
            debug_info.push_str(&format!("Error executing ccusage: {}\n", e));
        }
    }
    
    debug_info
}

async fn refresh_session_data(app_handle: &tauri::AppHandle) {
    // Set refresh flag
    IS_REFRESHING.store(true, Ordering::Relaxed);
    
    // Fetch active session data
    let active_block = fetch_session_data().await;
    
    // Update tray title with cost if there's an active session
    let title = if let Some(ref block) = active_block {
        format!("${:.2}", block.cost_usd)
    } else {
        String::new()
    };
    
    // Update cache
    {
        let mut cache = SESSION_CACHE.lock().unwrap();
        cache.active_block = active_block;
        cache.last_updated = Some(Instant::now());
    }
    
    // Update tray title
    if let Some(tray) = app_handle.tray_by_id("main") {
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
    let (active_block, has_attempted_fetch) = {
        let cache = SESSION_CACHE.lock().unwrap();
        (cache.active_block.clone(), cache.last_updated.is_some())
    };

    // Current session section
    let session_title = MenuItemBuilder::with_id("session_title", "Current session")
        .enabled(false)
        .build(app)?;
    menu_builder = menu_builder.item(&session_title);

    if let Some(block) = active_block {
        // Cost and token counts
        let input_k = block.token_counts.input_tokens as f64 / 1000.0;
        let output_k = block.token_counts.output_tokens as f64 / 1000.0;
        let cost_str = format!("Cost: ${:.2}", block.cost_usd);
        let tokens_str = format!("Tokens: In {:.1}K / Out {:.1}K", input_k, output_k);
        
        let cost_item = MenuItemBuilder::with_id("session_cost", &cost_str)
            .build(app)?;
        let tokens_item = MenuItemBuilder::with_id("session_tokens", &tokens_str)
            .build(app)?;
        menu_builder = menu_builder.item(&cost_item).item(&tokens_item);
        
        // Session times
        let start_time = chrono::DateTime::parse_from_rfc3339(&block.start_time)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Local).format("%I:%M %p").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
            
        let end_time = chrono::DateTime::parse_from_rfc3339(&block.end_time)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Local).format("%I:%M %p").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
            
        let session_start_item = MenuItemBuilder::with_id("session_start", &format!("Started: {}", start_time))
            .build(app)?;
        let session_end_item = MenuItemBuilder::with_id("session_end", &format!("Expires: {}", end_time))
            .build(app)?;
        menu_builder = menu_builder.item(&session_start_item).item(&session_end_item);
        
        // Models used
        if !block.models.is_empty() {
            menu_builder = menu_builder.separator();
            let models_header = MenuItemBuilder::with_id("models_header", "Models used")
                .enabled(false)
                .build(app)?;
            menu_builder = menu_builder.item(&models_header);
            
            for model in &block.models {
                let model_name = format_model_name(model);
                let model_item = MenuItemBuilder::with_id(
                    &format!("model_{}", model),
                    &model_name,
                )
                .build(app)?;
                menu_builder = menu_builder.item(&model_item);
            }
        }
        
        menu_builder = menu_builder.separator();
    } else if has_attempted_fetch {
        // We've tried to fetch but got no data - likely ccusage is not available
        let no_session = MenuItemBuilder::with_id("no_session", "No active session")
            .build(app)?;
        menu_builder = menu_builder.item(&no_session);
        
        // Add helpful error message
        let error_msg = MenuItemBuilder::with_id("error_msg", "ccusage may not be installed")
            .enabled(false)
            .build(app)?;
        menu_builder = menu_builder.item(&error_msg);
        
        let install_msg = MenuItemBuilder::with_id("install_msg", "Install: npm install -g ccusage")
            .build(app)?;
        menu_builder = menu_builder.item(&install_msg).separator();
    } else {
        // Still loading
        let loading = MenuItemBuilder::with_id("loading", "Loading...")
            .enabled(false)
            .build(app)?;
        menu_builder = menu_builder.item(&loading).separator();
    }


    // Refresh button
    let refresh = MenuItemBuilder::with_id("refresh", "Refresh")
        .build(app)?;
    menu_builder = menu_builder.item(&refresh);

    // Debug info (useful for troubleshooting)
    let debug = MenuItemBuilder::with_id("debug", "Debug Info")
        .build(app)?;
    menu_builder = menu_builder.item(&debug).separator();

    // Quit
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .accelerator("Cmd+Q")
        .build(app)?;
    menu_builder = menu_builder.item(&quit);

    Ok(menu_builder.build()?)
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![])
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
                        // Get initial title from cache
                        let initial_title = {
                            let cache = SESSION_CACHE.lock().unwrap();
                            cache.active_block.as_ref()
                                .map(|block| format!("${:.2}", block.cost_usd))
                        };
                        
                        let tray = TrayIconBuilder::with_id("main")
                            .icon(
                                tauri::image::Image::from_bytes(include_bytes!("../icons/bars.png"))
                                    .unwrap()
                                    .to_owned(),
                            )
                            .icon_as_template(true)
                            .title(initial_title.unwrap_or_default())
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
                                    "install_msg" => {
                                        let _ = tauri_plugin_opener::open_url(
                                            "https://github.com/ryoppippi/ccusage#installation",
                                            None::<String>,
                                        );
                                    }
                                    "quit" => {
                                        app.exit(0);
                                    }
                                    "refresh" => {
                                        let app_handle = app.app_handle().clone();
                                        tauri::async_runtime::spawn(async move {
                                            // Force refresh all data
                                            refresh_session_data(&app_handle).await;
                                            
                                            // Rebuild menu with fresh data
                                            if let Ok(new_menu) = build_menu(&app_handle).await {
                                                if let Some(tray) = app_handle.try_state::<Arc<tauri::tray::TrayIcon>>() {
                                                    let _ = tray.set_menu(Some(new_menu));
                                                }
                                            }
                                        });
                                    }
                                    "debug" => {
                                        tauri::async_runtime::spawn(async move {
                                            let debug_info = get_debug_info().await;
                                            println!("=== DEBUG INFO ===\n{}\n==================", debug_info);
                                            
                                            // Also try to show in a dialog if possible
                                            #[cfg(target_os = "macos")]
                                            {
                                                use std::process::Command as StdCommand;
                                                let _ = StdCommand::new("osascript")
                                                    .args(&[
                                                        "-e",
                                                        &format!(
                                                            r#"display dialog "{}" buttons {{"OK"}} default button "OK" with title "CCUsage Debug Info""#,
                                                            debug_info.replace("\"", "\\\"").replace("\n", "\\n")
                                                        ),
                                                    ])
                                                    .spawn();
                                            }
                                        });
                                    }
                                    _ => {}
                                }
                            })
                            .build(&app_handle)
                            .unwrap();

                        // Store tray reference in app state
                        app_handle.manage(Arc::new(tray));
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