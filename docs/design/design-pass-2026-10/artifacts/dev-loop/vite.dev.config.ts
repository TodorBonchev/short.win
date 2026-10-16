// Dev-server wrapper for the design pass (gitignored): the real config plus a `process.env.NODE_ENV`
// define, which the production build does on its own but the dev server leaves undefined for the
// workspace client's TypeScript source (clients/js/src/generated/errors/shortwin.ts).
import { defineConfig, mergeConfig } from 'vite';
import real from '../vite.config.ts';

export default defineConfig(async (env) => {
    const base = typeof real === 'function' ? await real(env) : real;
    return mergeConfig(base, { define: { 'process.env.NODE_ENV': JSON.stringify('development') } });
});
