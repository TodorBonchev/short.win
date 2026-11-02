import {defineConfig} from 'vite';
// Codama error helpers use process.env["NODE_ENV"]; expose only this build constant.
export default defineConfig(({mode})=>({define:{'process.env':JSON.stringify({NODE_ENV:mode})}}));
