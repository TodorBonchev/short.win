import path from 'node:path';
import {fileURLToPath} from 'node:url';

export const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const videoOutput = path.resolve(process.env.SHORTWIN_VIDEO_OUTPUT || path.join(projectRoot, 'exports'));
