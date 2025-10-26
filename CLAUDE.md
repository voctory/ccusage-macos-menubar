# CC Usage macOS Menubar App

## Overview

This is a minimal Tauri v2 application that runs as a menubar-only app on macOS. The app:
- Runs without a visible window on startup
- Shows only in the macOS menubar (system tray)
- Does not appear in the dock
- Displays real-time Codex usage data via `@ccusage/codex` (ccusage)
- Shows daily usage costs per model

## Architecture

### Backend (Rust)
- **src-tauri/src/lib.rs**: Main application logic
  - Sets up the system tray with dynamic menu items
  - Integrates with `@ccusage/codex` via `npx @ccusage/codex@latest daily --json`
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
- **@ccusage/codex CLI**: Required external dependency
  - Install with: `npm i -g @ccusage/codex` or use `npx @ccusage/codex@latest`
  - Provides Codex usage analytics

## Key Features

1. **System Tray Integration**
   - Icon appears in macOS menubar
   - Left-click shows menu with usage stats and options
   - No window interface - pure menubar app

2. **Today Display**
   - **Today** shows aggregated usage for the current day
   - **Cost** and **Token counts** (Input/Output) displayed
   - **Models used** header with each model listed separately
   - **Total cost** displayed in the menubar (e.g., $0.00 when no usage)
   - **"No usage today"** displayed when the day has no usage entry
   - **Refresh** (manually update all data)
   - **Launch on startup** (checkbox, toggles autostart)
   - **Quit** (with Cmd+Q shortcut)

3. **Error States**
   - **Install @ccusage/codex CLI** (clickable link to npm when CLI not found)
   - **No usage data** (when no conversations today)
   - Graceful fallback to cached data on network issues

4. **Launch on Startup**
   - Uses Tauri's autostart plugin
   - Toggleable via menu checkbox
   - Works across macOS, Windows, Linux

5. **Data Integration**
   - **Today**: `npx @ccusage/codex@latest daily --json`
   - Shows only today's aggregate usage
   - Caches data to handle network issues
   - Auto-formats model names where possible
   - Shows costs formatted as currency
   - Handles no-usage days gracefully (shows $0.00)

6. **Smart Refresh & Performance**
   - **Periodic refresh** every 2 minutes
   - **Smart caching** to avoid unnecessary fetches
   - **No menu interruption** - menu stays open during refresh
   - **Manual refresh** button forces immediate update
   - **Fast startup** with cached data

7. **macOS Specific**
   - Uses `ActivationPolicy::Accessory` to hide from dock
   - Icon adapts to light/dark mode with `icon_as_template(true)`

## Build & Run

### Prerequisites
1. **Install @ccusage/codex CLI** (required for usage data):
   ```bash
   npm i -g @ccusage/codex
   # OR use npx (no installation needed):
   npx @ccusage/codex@latest --help
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
1. **First time setup**: Ensure CLI works: `npx @ccusage/codex@latest daily --json`
2. **Run the app**: `yarn tauri dev` - no window should appear
3. **Find the icon**: Look for the app icon in the macOS menubar
4. **Test menu**: Left-click the icon to see usage data
5. **Test refresh**: Click "Refresh" to update data
6. **Test error state**: If `@ccusage/codex` isn't available, click "Install @ccusage/codex CLI"
7. **Verify dock**: The app shouldn't appear in the dock

### Menu Examples

**Normal state** (with usage today):
```
CCUsage
────────────────
Today
Cost: $17.59
Tokens: In 6.5K / Out 5.5K
────────────────
Models used
GPT‑5 Codex
────────────────
☑ Launch on startup
Refresh
────────────────
Quit
```

**No usage today**:
```
CCUsage
────────────────
Today
No usage today
────────────────
☑ Launch on startup
Refresh
────────────────
Quit
```

**Benefits**:
- **Clear cost display** - total cost visible in menubar (shows $0.00 for no-usage days)
- **Simple mental model** - focuses on today's usage only
- **Graceful handling** - shows "No usage today" when appropriate
- **Smooth user experience** - no lag or jitter

## Notes

- The app uses Tauri v2's tray icon API
- Window size is set to 400x300 when created
- The tray icon uses the default app icon from `src-tauri/icons/`
- Frontend can be expanded with any React components as needed

## GitHub Actions Release Workflow

The project includes a GitHub Actions workflow for automated releases.

### Using the Release Workflow
No Apple Developer account required. To create a release:

1. Go to the Actions tab on GitHub
2. Select "Release" workflow
3. Click "Run workflow"
4. Enter version (e.g., `1.0.0` - without v prefix)
5. The workflow will:
   - Update version in Cargo.toml and tauri.conf.json
   - Build for Apple Silicon (ARM64) only
   - Create a draft release with the DMG file
   - Note: Users will need to bypass Gatekeeper on first run

### Requirements
- Targets Apple Silicon Macs only (M1/M2/M3)
- No code signing (see "First Run Instructions" below)
- Requires Node.js installed on the user's machine for `@ccusage/codex` functionality

### First Run Instructions
Since the app is not code-signed, macOS may show "app is damaged and can't be opened" when downloading from GitHub releases.

**To fix this:**
```bash
# After moving the app to /Applications, run:
xattr -cr /Applications/ccusage-macos-menubar.app
```

**Alternative method:**
1. Right-click the app and select "Open"
2. Click "Open" in the security dialog

This only needs to be done once. The app is safe - macOS shows this for all unsigned apps downloaded from the internet.

### Manual Release Process
If you prefer to build and release manually:
```bash
# Build for Apple Silicon (ARM64)
yarn tauri build -- --target aarch64-apple-darwin

# Output will be in:
# src-tauri/target/release/bundle/dmg/
```
