/**
 * The web app's RFC 9116 `security.txt` (`public/.well-known/security.txt`; M10 SECTXT-4), checked
 * by the build (`vite.config.ts`), the unit test, the E2E `frame` spec and the deploy check
 * (`scripts/check-headers.ts`). The program embeds its own security.txt with the same owner values
 * (`programs/shortwin/src/lib.rs`, `scripts/check-security-txt.py`; the `complete_curve` helper is
 * retired, design pump-final D2).
 *
 * The owner fields held `OWNER-FILL` placeholders until the public identity was decided (M10 owner
 * question Q3, answered 2026-10-02: `config/securityPolicy.ts`). A non-production check (every other
 * build, the E2E suite) requires every field and its format, but lets a placeholder pass and does not
 * judge `Expires` against the clock, so a localnet / devnet build or a rebuild of an old commit never
 * starts failing as the date nears. A
 * production check (a `VITE_CLUSTER=mainnet` build, a deployed origin) refuses placeholders and an
 * `Expires` that is 30 days or less, or more than a year, ahead.
 *
 * An OpenPGP cleartext-signed file (RFC 9116 §2.3, RFC 4880 §7) is checked on its signed text: the
 * envelope is required to be well formed, but the signature itself is not verified here.
 * Pure: no I/O.
 */

/** Where the file is served (RFC 9116 §3). */
export const SECURITY_TXT_PATH = '/.well-known/security.txt';

/** The `Content-Type` the static host must send for it (RFC 9116 §3). */
export const SECURITY_TXT_CONTENT_TYPE = 'text/plain; charset=utf-8';

/** The marker of a value the owner has not filled yet. */
export const SECURITY_TXT_PLACEHOLDER = 'OWNER-FILL';

/** `Expires` must lie more than this many days ahead (a tripwire well before it lapses). */
export const EXPIRES_MIN_DAYS = 30;

/** The fields of the IANA "security.txt Fields" registry (RFC 9116 §6.4; CSAF 2.0 §7.1.8). */
const KNOWN = [
    'Acknowledgments',
    'Canonical',
    'Contact',
    'CSAF',
    'Encryption',
    'Expires',
    'Hiring',
    'Policy',
    'Preferred-Languages',
] as const;

const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;
const DAY_MS = 86_400_000;

export interface SecurityTxtCheck {
    /** The check's clock (a production check judges `Expires` against it). */
    readonly now: Date;
    /**
     * Refuse `OWNER-FILL` placeholders and an `Expires` outside (now + 30 days, now + 1 year]
     * (a mainnet build, a deployed origin).
     */
    readonly production: boolean;
}

const PGP_SIGNED = '-----BEGIN PGP SIGNED MESSAGE-----';
const PGP_SIGNATURE = '-----BEGIN PGP SIGNATURE-----';
const PGP_END = '-----END PGP SIGNATURE-----';

/**
 * The lines to parse, as [1-based line number, text]: the whole file, or — when it is OpenPGP
 * cleartext-signed (RFC 4880 §7) — the dash-unescaped signed text between the armor headers and
 * the signature. A malformed envelope is a problem.
 */
function bodyLines(text: string, problems: string[]): [number, string][] {
    const lines = text.split(/\r?\n/).map((l, i): [number, string] => [i + 1, l]);
    if (lines[0]?.[1].trimEnd() !== PGP_SIGNED) return lines;
    let i = 1;
    for (; i < lines.length && lines[i]?.[1].trim() !== ''; i++) {
        const [n, l] = lines[i] ?? [0, ''];
        if (!/^Hash: [A-Za-z0-9, ]+$/.test(l)) problems.push(`line ${n}: unexpected OpenPGP armor header ${l}`);
    }
    const sig = lines.findIndex(([, l], j) => j > i && l.trimEnd() === PGP_SIGNATURE);
    if (sig < 0) {
        problems.push(`a cleartext-signed file needs a ${PGP_SIGNATURE} block`);
        return [];
    }
    if (!lines.slice(sig + 1).some(([, l]) => l.trimEnd() === PGP_END)) problems.push(`${PGP_END} is missing`);
    return lines.slice(i + 1, sig).map(([n, l]): [number, string] => [n, l.startsWith('- ') ? l.slice(2) : l]);
}

function isHttps(v: string): boolean {
    try {
        return new URL(v).protocol === 'https:';
    } catch {
        return false;
    }
}

/** The problems of a security.txt body; an empty list when it is valid. */
export function securityTxtProblems(text: string, { now, production }: SecurityTxtCheck): string[] {
    const problems: string[] = [];
    const fields = new Map<string, string[]>();
    for (const [n, line] of bodyLines(text, problems)) {
        if (line.trim() === '' || line.startsWith('#')) continue;
        const m = /^([A-Za-z0-9-]+):[ \t]*(.*?)[ \t]*$/.exec(line);
        if (!m?.[1] || !m[2]) {
            problems.push(`line ${n} is neither a comment nor a "Field: value" line`);
            continue;
        }
        const name = KNOWN.find((k) => k.toLowerCase() === m[1]?.toLowerCase());
        if (!name) {
            problems.push(`line ${n}: unknown field ${m[1]}`);
            continue;
        }
        fields.set(name, [...(fields.get(name) ?? []), m[2]]);
    }
    const values = (name: (typeof KNOWN)[number]): string[] => fields.get(name) ?? [];
    for (const [name, vs] of fields) {
        for (const v of vs) {
            if (v.includes(SECURITY_TXT_PLACEHOLDER) && production) {
                problems.push(`${name} is still an ${SECURITY_TXT_PLACEHOLDER} placeholder`);
            }
        }
    }
    const filled = (v: string): boolean => !v.includes(SECURITY_TXT_PLACEHOLDER);

    const contacts = values('Contact');
    if (contacts.length === 0) problems.push('Contact is required');
    for (const c of contacts.filter(filled)) {
        if (!/^mailto:[^@\s]+@[^@\s]+$/.test(c) && !isHttps(c)) problems.push(`Contact ${c} is not mailto: or https:`);
    }

    const expires = values('Expires');
    if (expires.length !== 1) problems.push(`Expires must appear exactly once (found ${expires.length})`);
    for (const e of expires.filter(filled)) {
        const t = Date.parse(e);
        if (!RFC3339.test(e) || Number.isNaN(t)) {
            problems.push(`Expires ${e} is not an RFC 3339 date-time`);
            continue;
        }
        // The window only in production: elsewhere the file's age must not fail a build.
        if (!production) continue;
        const latest = new Date(now);
        latest.setUTCFullYear(latest.getUTCFullYear() + 1);
        if (t <= now.getTime() + EXPIRES_MIN_DAYS * DAY_MS) {
            problems.push(`Expires ${e} is not more than ${EXPIRES_MIN_DAYS} days ahead`);
        } else if (t > latest.getTime()) {
            problems.push(`Expires ${e} is more than a year ahead (RFC 9116 §2.5.5)`);
        }
    }

    for (const name of ['Canonical', 'Policy'] as const) {
        const vs = values(name);
        if (vs.length === 0) problems.push(`${name} is required`);
        for (const v of vs.filter(filled)) if (!isHttps(v)) problems.push(`${name} ${v} is not an https URL`);
    }
    for (const v of values('Canonical').filter(filled)) {
        if (isHttps(v) && new URL(v).pathname !== SECURITY_TXT_PATH) {
            problems.push(`Canonical ${v} is not served at ${SECURITY_TXT_PATH}`);
        }
    }
    if (values('Preferred-Languages').length > 1) problems.push('Preferred-Languages must appear at most once');
    return problems;
}
