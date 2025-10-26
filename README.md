# ccusage-macos-menubar

Small macOS menubar wrapping the [ccusage CLI](https://github.com/ryoppippi/ccusage) via the `@ccusage/codex` package to show today's Claude Code usage and cost.

Menubar data auto-refreshes every 2 minutes in the background, or you can manually hit "Refresh".

Example CLI used by the app:

```
npx @ccusage/codex@latest daily --json
```

<img src="./screenshot_codex.png" width="343">

Built with [Tauri](https://v2.tauri.app/).
