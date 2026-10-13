/**
 * Renders `public/icon-192.png` from `public/favicon.svg` (FRONTEND.md §5.4 (6)): the icon the
 * Mobile Wallet Adapter's `appIdentity` names (`icon-192.png`, relative to the origin). Android
 * wallets may not render SVG, so it is a 192 × 192 PNG.
 *
 *   node scripts/icon-192.ts           # writes public/icon-192.png
 *   node scripts/icon-192.ts --check   # exits 1 when the file is not the rendering of favicon.svg
 *
 * The favicon is two shapes, and this renders exactly those: the rounded square (`rect` with `rx`)
 * and the stroked polyline (`path` of `M` / `l` commands, round caps and joins, i.e. every point
 * within half the stroke width of a segment), 8 × 8 samples per pixel, transparent outside the
 * square, encoded as an 8-bit RGBA PNG with Node's zlib. Deterministic: the same SVG gives the same
 * pixels. `--check` and `mobileWalletAdapter.test.ts` compare the committed file's decoded pixels,
 * not its bytes: another zlib may compress the same image differently. A favicon with any other
 * shape fails here instead of rendering something else.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { crc32, deflateSync, inflateSync } from 'node:zlib';

export const ICON_SIZE = 192;
const SAMPLES = 8;
const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a] as const;

type Rgb = readonly [number, number, number];
type Point = readonly [number, number];

function attr(tag: string, name: string): string {
    const m = new RegExp(`\\s${name}="([^"]*)"`).exec(tag);
    if (!m?.[1]) throw new Error(`favicon.svg: ${name} missing in ${tag}`);
    return m[1];
}

function hex(c: string): Rgb {
    const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(c);
    if (!m?.[1] || !m[2] || !m[3]) throw new Error(`favicon.svg: unsupported colour ${c}`);
    return [Number.parseInt(m[1], 16), Number.parseInt(m[2], 16), Number.parseInt(m[3], 16)];
}

/** The favicon's two shapes, read from the SVG (anything else is refused). */
export function parseFavicon(svg: string) {
    const tags = svg.match(/<(rect|path)\b[^>]*>/g) ?? [];
    if (tags.length !== 2 || !tags[0]?.startsWith('<rect') || !tags[1]?.startsWith('<path')) {
        throw new Error('favicon.svg: expected exactly one <rect> then one <path>');
    }
    if (/<(circle|ellipse|line|polyline|polygon|g|image|text|use)\b/.test(svg)) {
        throw new Error('favicon.svg: unexpected element');
    }
    const view = attr(/<svg\b[^>]*>/.exec(svg)?.[0] ?? '', 'viewBox')
        .split(/\s+/)
        .map(Number);
    const [rect, path] = tags as [string, string];
    const size = Number(attr(rect, 'width'));
    if (view.join(' ') !== `0 0 ${size} ${size}` || Number(attr(rect, 'height')) !== size) {
        throw new Error('favicon.svg: the square must fill the viewBox');
    }
    if (attr(path, 'fill') !== 'none' || attr(path, 'stroke-linecap') !== 'round') {
        throw new Error('favicon.svg: the path must be an unfilled stroke with round caps');
    }
    if (attr(path, 'stroke-linejoin') !== 'round') throw new Error('favicon.svg: round joins expected');
    const d = attr(path, 'd');
    const cmds = [...d.matchAll(/([MLml])([^MLml]*)/g)];
    if (cmds.map((c) => c[0]).join('') !== d.replace(/\s+$/, '')) throw new Error(`favicon.svg: unsupported path ${d}`);
    const points: Point[] = [];
    for (const [, cmd, args] of cmds) {
        // SVG numbers need no separator before a sign ("4-4" is 4 then -4).
        const n = ((args ?? '').match(/-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/gi) ?? []).map(Number);
        if (n.length === 0 || n.length % 2 !== 0 || n.some((v) => !Number.isFinite(v))) {
            throw new Error(`favicon.svg: bad path ${d}`);
        }
        for (let i = 0; i < n.length; i += 2) {
            const prev = points.at(-1) ?? [0, 0];
            const x = n[i] as number;
            const y = n[i + 1] as number;
            const rel = cmd === 'l' || (cmd === 'm' && points.length > 0);
            points.push(rel ? [prev[0] + x, prev[1] + y] : [x, y]);
        }
    }
    return {
        size,
        radius: Number(attr(rect, 'rx')),
        fill: hex(attr(rect, 'fill')),
        stroke: hex(attr(path, 'stroke')),
        halfWidth: Number(attr(path, 'stroke-width')) / 2,
        points,
    };
}

function segmentDistance2(px: number, py: number, a: Point, b: Point): number {
    const dx = b[0] - a[0];
    const dy = b[1] - a[1];
    const len2 = dx * dx + dy * dy;
    const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((px - a[0]) * dx + (py - a[1]) * dy) / len2));
    const x = a[0] + t * dx - px;
    const y = a[1] + t * dy - py;
    return x * x + y * y;
}

/** The RGBA pixels (row-major, 4 bytes each) of the favicon at `ICON_SIZE` px. */
export function renderPixels(svg: string): Uint8Array {
    const f = parseFavicon(svg);
    const scale = f.size / ICON_SIZE;
    const out = new Uint8Array(ICON_SIZE * ICON_SIZE * 4);
    const r = f.radius;
    const w2 = f.halfWidth * f.halfWidth;
    const segments = f.points.slice(1).map((p, i) => [f.points[i] as Point, p] as const);
    for (let py = 0; py < ICON_SIZE; py++) {
        for (let px = 0; px < ICON_SIZE; px++) {
            let a = 0;
            const sum = [0, 0, 0];
            for (let j = 0; j < SAMPLES; j++) {
                for (let i = 0; i < SAMPLES; i++) {
                    const x = (px + (i + 0.5) / SAMPLES) * scale;
                    const y = (py + (j + 0.5) / SAMPLES) * scale;
                    const cx = Math.min(Math.max(x, r), f.size - r);
                    const cy = Math.min(Math.max(y, r), f.size - r);
                    if ((x - cx) ** 2 + (y - cy) ** 2 > r * r) continue; // outside the rounded square
                    const onStroke = segments.some(([p, q]) => segmentDistance2(x, y, p, q) <= w2);
                    const c = onStroke ? f.stroke : f.fill;
                    a += 1;
                    sum[0] = (sum[0] as number) + c[0];
                    sum[1] = (sum[1] as number) + c[1];
                    sum[2] = (sum[2] as number) + c[2];
                }
            }
            const o = (py * ICON_SIZE + px) * 4;
            if (a > 0) {
                out[o] = Math.round((sum[0] as number) / a);
                out[o + 1] = Math.round((sum[1] as number) / a);
                out[o + 2] = Math.round((sum[2] as number) / a);
                out[o + 3] = Math.round((a / (SAMPLES * SAMPLES)) * 255);
            }
        }
    }
    return out;
}

function chunk(type: string, data: Uint8Array): Buffer {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(data.length, 0);
    head.write(type, 4, 'ascii');
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])) >>> 0, 0);
    return Buffer.concat([head, data, crc]);
}

/** The PNG file (8-bit RGBA, no filter, zlib level 9). */
export function renderIcon192(svg: string): Uint8Array {
    const pixels = renderPixels(svg);
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(ICON_SIZE, 0);
    ihdr.writeUInt32BE(ICON_SIZE, 4);
    ihdr.set([8, 6, 0, 0, 0], 8);
    const raw = Buffer.alloc(ICON_SIZE * (ICON_SIZE * 4 + 1));
    for (let y = 0; y < ICON_SIZE; y++) {
        raw[y * (ICON_SIZE * 4 + 1)] = 0;
        raw.set(pixels.subarray(y * ICON_SIZE * 4, (y + 1) * ICON_SIZE * 4), y * (ICON_SIZE * 4 + 1) + 1);
    }
    return Buffer.concat([
        Buffer.from(PNG_SIGNATURE),
        chunk('IHDR', ihdr),
        chunk('IDAT', deflateSync(raw, { level: 9 })),
        chunk('IEND', new Uint8Array(0)),
    ]);
}

/**
 * The size and RGBA pixels of a PNG as `renderIcon192` writes it: 8-bit RGBA, not interlaced, every
 * row unfiltered (filter type 0). Checks each chunk's CRC; refuses anything else (another tool's
 * output is regenerated with `node scripts/icon-192.ts`).
 */
export function decodePng(png: Uint8Array): { width: number; height: number; pixels: Uint8Array } {
    const buf = Buffer.from(png.buffer, png.byteOffset, png.byteLength);
    if (buf.length < 8 || PNG_SIGNATURE.some((b, i) => buf[i] !== b)) throw new Error('PNG: bad signature');
    let ihdr: Buffer | null = null;
    const idat: Buffer[] = [];
    let at = 8;
    for (;;) {
        if (at + 12 > buf.length) throw new Error('PNG: truncated (no IEND)');
        const length = buf.readUInt32BE(at);
        const end = at + 8 + length;
        if (end + 4 > buf.length) throw new Error('PNG: truncated chunk');
        const type = buf.toString('ascii', at + 4, at + 8);
        if (crc32(buf.subarray(at + 4, end)) >>> 0 !== buf.readUInt32BE(end)) throw new Error(`PNG: bad CRC (${type})`);
        const data = buf.subarray(at + 8, end);
        if (type === 'IHDR') ihdr = data;
        else if (type === 'IDAT') idat.push(data);
        else if (type === 'IEND') break;
        at = end + 4;
    }
    if (ihdr?.length !== 13) throw new Error('PNG: no IHDR');
    const width = ihdr.readUInt32BE(0);
    const height = ihdr.readUInt32BE(4);
    // Bit depth 8, colour type 6 (RGBA), deflate, adaptive filtering, no interlace.
    if ([...ihdr.subarray(8)].join() !== '8,6,0,0,0') throw new Error('PNG: not 8-bit RGBA without interlace');
    const raw = inflateSync(Buffer.concat(idat));
    const stride = width * 4;
    if (raw.length !== height * (stride + 1)) throw new Error('PNG: image data of the wrong size');
    const pixels = new Uint8Array(height * stride);
    for (let y = 0; y < height; y++) {
        const row = y * (stride + 1);
        if (raw[row] !== 0) throw new Error(`PNG: row ${y} is filtered (type ${raw[row]})`);
        pixels.set(raw.subarray(row + 1, row + 1 + stride), y * stride);
    }
    return { width, height, pixels };
}

/** Whether `png` shows exactly the favicon's rendering at `ICON_SIZE` px (pixels, not bytes). */
export function isIconOf(png: Uint8Array, svg: string): boolean {
    const file = decodePng(png);
    return (
        file.width === ICON_SIZE &&
        file.height === ICON_SIZE &&
        Buffer.compare(Buffer.from(file.pixels), Buffer.from(renderPixels(svg))) === 0
    );
}

const isMain = !!process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isMain) {
    const pub = join(import.meta.dirname, '..', 'public');
    const svg = readFileSync(join(pub, 'favicon.svg'), 'utf8');
    const png = renderIcon192(svg);
    const file = join(pub, 'icon-192.png');
    if (process.argv.includes('--check')) {
        const same = isIconOf(readFileSync(file), svg);
        console.log(
            same ? 'icon-192.png is up to date' : 'icon-192.png differs from favicon.svg: run node scripts/icon-192.ts',
        );
        process.exit(same ? 0 : 1);
    }
    writeFileSync(file, png);
    console.log(`wrote ${file} (${png.length} bytes)`);
}
