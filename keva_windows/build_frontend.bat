@echo off
cd /d "%~dp0..\frontend"
pnpm install && pnpm build
