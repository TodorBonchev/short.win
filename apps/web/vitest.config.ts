import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
    plugins: [react()],
    test: {
        environment: 'jsdom',
        include: ['src/**/*.test.{ts,tsx}'],
        setupFiles: ['src/test/setup.ts'],
        env: {
            VITE_CLUSTER: 'localnet',
            VITE_RPC_URL: 'http://127.0.0.1:8899',
            VITE_RPC_WS_URL: 'ws://127.0.0.1:8900',
            VITE_INDEXER_URL: 'http://127.0.0.1:8080',
            VITE_INDEXER_WS_URL: 'ws://127.0.0.1:8080/v1/ws',
        },
    },
});
