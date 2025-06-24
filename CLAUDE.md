# CC Usage macOS Menubar App

## Overview

This is a minimal Tauri v2 application that runs as a menubar-only app on macOS. The app:
- Runs without a visible window on startup
- Shows only in the macOS menubar (system tray)
- Does not appear in the dock
- Displays real-time Claude Code usage data from ccusage CLI
- Shows daily usage costs per model (Opus 4, Sonnet 4, etc.)

## Architecture

### Backend (Rust)
- **src-tauri/src/lib.rs**: Main application logic
  - Sets up the system tray with dynamic menu items
  - Integrates with ccusage CLI via `npx ccusage@latest daily --json --breakdown`
  - Handles JSON parsing and data caching
  - Manages autostart functionality
  - Sets macOS activation policy to `Accessory` (no dock icon)

### Frontend (React)
- **src/App.tsx**: Minimal React component (not used in menubar-only mode)

### Configuration
- **Cargo.toml**: 
  - Enables `tray-icon`, `image-png` features
  - Includes `tokio` for async process execution
  - Includes `serde` for JSON parsing
- **tauri.conf.json**: 
  - Empty windows array (no window on startup)
  - Enables macOS private API for dock hiding

### Dependencies
- **ccusage CLI**: Required external dependency
  - Install with: `npm install -g ccusage` or use `npx ccusage@latest`
  - Provides Claude Code usage analytics

## Key Features

1. **System Tray Integration**
   - Icon appears in macOS menubar
   - Left-click shows menu with usage stats and options
   - No window interface - pure menubar app

2. **Dynamic Menu Content with Time Periods**
   - **CCUsage - [Period]** (submenu with Today/5 Hrs/1 Hr/Week options)
   - **Real-time model costs** (e.g., "Opus 4: $9.51", "Sonnet 4: $1.49") - now readable, not grayed out
   - **Refresh** (manually update usage data)
   - **Launch on startup** (checkbox, toggles autostart)
   - **Quit** (with Cmd+Q shortcut)

3. **Error States**
   - **Install ccusage CLI** (clickable link to GitHub when ccusage not found)
   - **No usage data** (when no conversations today)
   - Graceful fallback to cached data on network issues

4. **Launch on Startup**
   - Uses Tauri's autostart plugin
   - Toggleable via menu checkbox
   - Works across macOS, Windows, Linux

5. **Data Integration**
   - **Today**: `npx ccusage@latest daily --json --breakdown`
   - **5 Hrs/1 Hr**: `npx ccusage@latest blocks --json --breakdown --recent --session-length [1|5]`
   - **Week**: `npx ccusage@latest daily --json --breakdown --since [7-days-ago]`
   - Caches data to handle network issues
   - Auto-formats model names (claude-opus-4-20250514 → "Opus 4")
   - Shows costs formatted as currency ($9.51)
   - Aggregates weekly data across all days

6. **macOS Specific**
   - Uses `ActivationPolicy::Accessory` to hide from dock
   - Icon adapts to light/dark mode with `icon_as_template(true)`

## Build & Run

### Prerequisites
1. **Install ccusage CLI** (required for usage data):
   ```bash
   npm install -g ccusage
   # OR use npx (no installation needed):
   npx ccusage@latest --help
   ```

### Development
```bash
yarn
yarn tauri dev
```

### Production Build
```bash
yarn tauri build
```

## Testing

To verify the menubar behavior:
1. **First time setup**: Ensure ccusage CLI works: `npx ccusage@latest daily --json`
2. **Run the app**: `yarn tauri dev` - no window should appear
3. **Find the icon**: Look for the app icon in the macOS menubar
4. **Test menu**: Left-click the icon to see usage data
5. **Test refresh**: Click "Refresh" to update data
6. **Test error state**: If ccusage isn't available, click "Install ccusage CLI"
7. **Verify dock**: The app shouldn't appear in the dock

### Menu Examples

**Normal state** (with ccusage data):
```
▶ CCUsage - Today
  ├ Today
  ├ 5 Hrs  
  ├ 1 Hr
  └ Week
────────────────
Opus 4: $9.51       ← (readable, not grayed out)
Sonnet 4: $1.49     ← (readable, not grayed out)
────────────────
Refresh
────────────────
☑ Launch on startup
────────────────
Quit
```

**Error state** (no ccusage):
```
▶ CCUsage - Today
  ├ Today
  ├ 5 Hrs
  ├ 1 Hr
  └ Week
────────────────
Install ccusage CLI
────────────────
Refresh
────────────────
☑ Launch on startup
────────────────
Quit
```

**Different time periods**:
- **Today**: Shows today's usage by model
- **5 Hrs**: Shows usage from current 5-hour billing block  
- **1 Hr**: Shows usage from current 1-hour billing block
- **Week**: Shows aggregated usage from last 7 days

## Notes

- The app uses Tauri v2's tray icon API
- Window size is set to 400x300 when created
- The tray icon uses the default app icon from `src-tauri/icons/`
- Frontend can be expanded with any React components as needed