use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{TrayIconBuilder},
    Manager,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Instant;
use tokio::process::Command;
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionsResponse {
    sessions: Vec<BlockData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelStats {
    #[serde(rename = "isFallback")]
    is_fallback: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyEntry {
    date: String,
    #[serde(rename = "inputTokens")]
    input_tokens: u64,
    #[serde(rename = "cachedInputTokens")]
    cached_input_tokens: u64,
    #[serde(rename = "outputTokens")]
    output_tokens: u64,
    #[serde(rename = "totalTokens")]
    total_tokens: u64,
    #[serde(rename = "costUSD")]
    cost_usd: f64,
    models: HashMap<String, ModelStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyResponse {
    daily: Vec<DailyEntry>,
}

fn daily_to_block(entry: &DailyEntry) -> BlockData {
    // Convert an aggregated daily entry into a BlockData shape used by UI
    let token_counts = TokenCounts {
        input_tokens: entry.input_tokens,
        output_tokens: entry.output_tokens,
        // We don't have separate creation vs read; attribute all to read for display purposes
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: entry.cached_input_tokens,
    };

    let mut models: Vec<String> = entry.models.keys().cloned().collect();
    models.sort();

    BlockData {
        id: format!("daily-{}", entry.date),
        // Not available in daily output
        start_time: String::new(),
        end_time: String::new(),
        is_active: true,
        token_counts,
        cost_usd: entry.cost_usd,
        models,
    }
}


#[derive(Debug, Clone)]
struct SessionData {
    active_block: Option<BlockData>,
    last_updated: Option<Instant>,
    ccusage_available: bool,
}

static SESSION_CACHE: Mutex<SessionData> = Mutex::new(SessionData {
    active_block: None,
    last_updated: None,
    ccusage_available: false,
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
        "gpt-5-codex" => "GPT-5 Codex".to_string(),
        "gpt-5" => "GPT-5".to_string(),
        _ => {
            if model_name.contains("opus") {
                "Opus".to_string()
            } else if model_name.contains("sonnet") {
                "Sonnet".to_string()
            } else if model_name.contains("haiku") {
                "Haiku".to_string()
            } else if model_name.contains("gpt-5-codex") {
                "GPT-5 Codex".to_string()
            } else if model_name == "gpt-5" {
                "GPT-5".to_string()
            } else {
                model_name.to_string()
            }
        }
    }
}

async fn fetch_session_data() -> (Option<BlockData>, bool) {
    // Try multiple approaches to find and run CLI
    // Use login zsh so ~/.zprofile (Homebrew path, etc.) is loaded; avoid interactive ~/.zshrc
    let shell_commands = vec![
        ("/bin/zsh", vec![
            "-l",
            "-c",
            "NVM_DIR=\"${NVM_DIR:-$HOME/.nvm}\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; npm exec --yes @ccusage/codex@latest -- daily --json",
        ]),
        ("/bin/zsh", vec![
            "-l",
            "-c",
            "NVM_DIR=\"${NVM_DIR:-$HOME/.nvm}\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; npx @ccusage/codex@latest daily --json",
        ]),
        ("/bin/zsh", vec![
            "-l",
            "-c",
            "NVM_DIR=\"${NVM_DIR:-$HOME/.nvm}\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; ccusage daily --json",
        ]),
        // Fallbacks without login shell
        ("sh", vec!["-c", "ccusage daily --json"]),
        ("sh", vec!["-c", "npx @ccusage/codex@latest daily --json"]),
    ];

    for (cmd, args) in shell_commands {
        let output = Command::new(cmd)
            .args(&args)
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);

                // Try to parse the response with multiple schemas for compatibility
                if let Ok(response) = serde_json::from_str::<DailyResponse>(&stdout) {
                    // Prefer today's entry; if missing, show 0.00 for today
                    let today = chrono::Local::now().format("%b %d, %Y").to_string();
                    if let Some(entry) = response.daily.iter().find(|d| d.date == today) {
                        let block = daily_to_block(entry);
                        return (Some(block), true);
                    } else {
                        let zero = DailyEntry {
                            date: today,
                            input_tokens: 0,
                            cached_input_tokens: 0,
                            output_tokens: 0,
                            total_tokens: 0,
                            cost_usd: 0.0,
                            models: HashMap::new(),
                        };
                        let block = daily_to_block(&zero);
                        return (Some(block), true);
                    }
                }
                if let Ok(response) = serde_json::from_str::<SessionsResponse>(&stdout) {
                    let active_block = response
                        .sessions
                        .into_iter()
                        .find(|block| block.is_active);
                    return (active_block, true);
                }

                if let Ok(response) = serde_json::from_str::<BlocksResponse>(&stdout) {
                    let active_block = response
                        .blocks
                        .into_iter()
                        .find(|block| block.is_active);
                    return (active_block, true);
                }

                if let Ok(block) = serde_json::from_str::<BlockData>(&stdout) {
                    return (Some(block), true);
                }

                if let Ok(blocks) = serde_json::from_str::<Vec<BlockData>>(&stdout) {
                    let active_block = blocks.into_iter().find(|block| block.is_active);
                    return (active_block, true);
                }

                eprintln!("Failed to parse CLI response with known schemas");
                eprintln!("Response was: {}", stdout);
                continue;
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
    (None, false)
}

// Removed fetch_blocks_data and fetch_week_data functions as they are no longer needed

async fn get_debug_info() -> String {
    let mut debug_info = String::new();
    
    // Get PATH environment variable
    debug_info.push_str("Environment:\n");
    if let Ok(path) = std::env::var("PATH") {
        debug_info.push_str(&format!("Default PATH: {}\n", path));
    } else {
        debug_info.push_str("Default PATH: (not set)\n");
    }
    
    // Explain shell used for checks
    debug_info.push_str("Checks run in login zsh; nvm sourced if present\n\n");
    
    // Test commands with login zsh + optional nvm sourcing
    debug_info.push_str("Command availability (login zsh + nvm):\n");
    
    let commands_to_test = vec![
        ("which npx".to_string(), "npx location"),
        ("which node".to_string(), "node location"),
        ("which ccusage".to_string(), "ccusage location"),
        ("npx --version".to_string(), "npx version"),
        ("node --version".to_string(), "node version"),
        ("ccusage --version 2>&1 || echo 'not found'".to_string(), "ccusage version"),
    ];
    
    let nvm_source = r#"NVM_DIR="${NVM_DIR:-$HOME/.nvm}"; [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh""#;
    for (cmd, desc) in commands_to_test {
        let cmd_with_nvm = format!("{}; {}", nvm_source, cmd);
        let output = Command::new("/bin/zsh")
            .args(&["-l", "-c", &cmd_with_nvm])
            .output()
            .await;
            
        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                debug_info.push_str(&format!("{}: {}\n", desc, stdout.trim()));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.trim().is_empty() {
                    debug_info.push_str(&format!("{}: not found\n", desc));
                } else {
                    debug_info.push_str(&format!("{}: {}\n", desc, stderr.trim()));
                }
            }
            Err(e) => {
                debug_info.push_str(&format!("{}: error - {}\n", desc, e));
            }
        }
    }
    
    // Test @ccusage/codex with extended PATH
    debug_info.push_str("\nTesting @ccusage/codex:\n");
    let ccusage_cmd = format!(
        r#"{}; npm exec --yes @ccusage/codex@latest -- --version || npx @ccusage/codex@latest --version"#,
        nvm_source
    );
    let ccusage_output = Command::new("/bin/zsh")
        .args(&["-l", "-c", &ccusage_cmd])
        .output()
        .await;
        
    match ccusage_output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                debug_info.push_str(&format!("@ccusage/codex version: {}\n", stdout.trim()));
            } else {
                debug_info.push_str("@ccusage/codex: not available (npx @ccusage/codex@latest failed)\n");
                if !output.stderr.is_empty() {
                    debug_info.push_str(&format!("Error: {}\n", String::from_utf8_lossy(&output.stderr).trim()));
                }
            }
        }
        Err(e) => {
            debug_info.push_str(&format!("Error executing @ccusage/codex: {}\n", e));
        }
    }
    
    debug_info
}

async fn refresh_session_data(app_handle: &tauri::AppHandle) {
    // Set refresh flag
    IS_REFRESHING.store(true, Ordering::Relaxed);
    
    // Fetch active session data
    let (active_block, ccusage_available) = fetch_session_data().await;
    
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
        cache.ccusage_available = ccusage_available;
    }
    
    // Update tray title
    if let Some(tray) = app_handle.tray_by_id("main") {
        let _ = tray.set_title(Some(title));
    }
    
    // Rebuild and update the menu to reflect new data
    if let Ok(new_menu) = build_menu(app_handle).await {
        if let Some(tray) = app_handle.try_state::<Arc<tauri::tray::TrayIcon>>() {
            let _ = tray.set_menu(Some(new_menu));
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
    let (active_block, has_attempted_fetch, ccusage_available) = {
        let cache = SESSION_CACHE.lock().unwrap();
        (cache.active_block.clone(), cache.last_updated.is_some(), cache.ccusage_available)
    };

    // Today section
    let session_title = MenuItemBuilder::with_id("session_title", "Today")
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
        
        // Session times (only if available)
        let start_time = chrono::DateTime::parse_from_rfc3339(&block.start_time)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Local).format("%I:%M %p").to_string());
        let end_time = chrono::DateTime::parse_from_rfc3339(&block.end_time)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Local).format("%I:%M %p").to_string());

        if let Some(start) = start_time {
            let session_start_item = MenuItemBuilder::with_id("session_start", &format!("Started: {}", start))
                .build(app)?;
            menu_builder = menu_builder.item(&session_start_item);
        }
        if let Some(end) = end_time {
            let session_end_item = MenuItemBuilder::with_id("session_end", &format!("Expires: {}", end))
                .build(app)?;
            menu_builder = menu_builder.item(&session_end_item);
        }
        
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
        // We've tried to fetch
        let no_session = MenuItemBuilder::with_id("no_session", "No usage today")
            .build(app)?;
        menu_builder = menu_builder.item(&no_session);
        
        // Only show error if ccusage is actually not available
        if !ccusage_available {
            // Add helpful error message
            let error_msg = MenuItemBuilder::with_id("error_msg", "@ccusage/codex may not be installed")
                .enabled(false)
                .build(app)?;
            menu_builder = menu_builder.item(&error_msg);
            
            let install_msg = MenuItemBuilder::with_id("install_msg", "Install: npm i -g @ccusage/codex")
                .build(app)?;
            menu_builder = menu_builder.item(&install_msg);
        }
        
        menu_builder = menu_builder.separator();
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
                                            "https://www.npmjs.com/package/@ccusage/codex",
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
