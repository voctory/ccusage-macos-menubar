# CC Usage macOS Menubar App

## Overview

This is a minimal Tauri v2 application that runs as a menubar-only app on macOS. The app:
- Runs without a visible window on startup
- Shows only in the macOS menubar (system tray)
- Does not appear in the dock
- Can show a window when clicked or via menu

## Architecture

### Backend (Rust)
- **src-tauri/src/lib.rs**: Main application logic
  - Sets up the system tray with menu items
  - Handles tray events (clicks, menu selections)
  - Manages window creation/visibility
  - Sets macOS activation policy to `Accessory` (no dock icon)

### Frontend (React)
- **src/App.tsx**: Minimal React component
  - Simple UI shown when window is opened
  - Demonstrates Tauri command invocation

### Configuration
- **Cargo.toml**: Enables `tray-icon` feature
- **tauri.conf.json**: 
  - Empty windows array (no window on startup)
  - Enables macOS private API for dock hiding

## Key Features

1. **System Tray Integration**
   - Icon appears in macOS menubar
   - Right-click shows menu with "Show" and "Quit" options
   - Left-click opens/focuses the window

2. **Window Management**
   - No window shown on app launch
   - Window created on-demand when user clicks tray or selects "Show"
   - Window is reused if already created

3. **macOS Specific**
   - Uses `ActivationPolicy::Accessory` to hide from dock
   - Supports Cmd+Q shortcut for quit

## Build & Run

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
1. Run the app - no window should appear
2. Look for the app icon in the menubar
3. Click the icon - a window should appear
4. Right-click for menu options
5. Verify the app doesn't show in the dock

## Notes

- The app uses Tauri v2's tray icon API
- Window size is set to 400x300 when created
- The tray icon uses the default app icon from `src-tauri/icons/`
- Frontend can be expanded with any React components as needed