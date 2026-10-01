import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'node:path'

// Tauri 개발 서버 설정 — 1420·1421·1430·1440 은 다른 앱이 쓴다
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
  clearScreen: false,
  server: {
    port: 1450,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  build: {
    target: 'chrome110',
    sourcemap: false,
    // 화면(index.html)과 숨은 출력 창(print.html) 두 쪽
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, 'index.html'),
        print: path.resolve(__dirname, 'print.html'),
      },
    },
  },
})
