# Building Keva for Windows

This document describes how to build the Keva Windows application.

## Prerequisites

- **Rust** (stable toolchain)
- **Node.js** (v18+)
- **pnpm** - Install via `npm install -g pnpm`

## Build Steps

### 1. Build the WebView Frontend

The WebView UI is built using Vite. This step bundles Monaco editor and other dependencies.

```sh
cd keva_windows
build_frontend.bat
```

Or manually:

```sh
cd frontend
pnpm install
pnpm build
```

This creates a `dist/` folder containing the bundled assets.

### 2. Build the Rust Application

```sh
cargo build --release -p keva_windows
```

The executable will be at `target/release/keva_windows.exe`.

## Development

For frontend development with hot reload:

```sh
cd frontend
pnpm dev
```

Note: The Rust application loads from `dist/`, so you need to run `pnpm build` after making frontend changes to see them in the app.

## Project Structure

```
keva/
├── frontend/               # Shared Vite project (used by Windows and macOS)
│   ├── package.json        # npm dependencies
│   ├── vite.config.ts      # Vite bundler configuration
│   ├── index.html          # Entry point
│   ├── src/                # Source files (ES modules)
│   │   ├── main.js         # Application entry point
│   │   ├── editor.js       # Monaco editor wrapper
│   │   ├── styles.css      # Styles
│   │   └── ...
│   ├── dist/               # Build output (gitignored)
│   └── node_modules/       # Dependencies (gitignored)
└── keva_windows/src/webview/
    └── init.rs             # WebView2 initialization (Rust)
```

## Troubleshooting

**Monaco editor not loading**: Ensure `pnpm build` completed successfully and `dist/` folder exists.

**Blank window**: Check browser dev tools (F12 in debug builds) for JavaScript errors.
