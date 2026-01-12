@echo off
cd /d "%~dp0src\webview\vite"
pnpm install && pnpm build
