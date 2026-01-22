import { defineConfig } from 'vite';
import monacoEditorPlugin from 'vite-plugin-monaco-editor';

export default defineConfig({
  base: './',
  plugins: [
    (monacoEditorPlugin as any).default({
      languageWorkers: ['editorWorkerService'],
      customWorkers: [],
      languages: ['markdown'],
    }),
  ],
  build: {
    outDir: 'dist',
    emptyDirBeforeWrite: true,
    rollupOptions: {
      output: {
        manualChunks: {
          monaco: ['monaco-editor'],
        },
      },
    },
  },
});
