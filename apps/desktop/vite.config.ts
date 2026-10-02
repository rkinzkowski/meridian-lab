import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';

export default defineConfig({
  plugins: [react()],
  resolve: {alias: {'@meridian/shared-types': fileURLToPath(new URL('../../packages/shared-types/src/index.ts', import.meta.url)), '@meridian/maps': fileURLToPath(new URL('../../packages/maps/src/index.ts', import.meta.url))}},
  server: {host: '127.0.0.1', port: 1420, strictPort: true},
  clearScreen: false,
  build: {target: 'es2022', chunkSizeWarningLimit: 1500},
  test: {environment: 'jsdom', include: ['src/**/*.test.{ts,tsx}', '../../packages/**/*.test.ts'], restoreMocks: true},
});
