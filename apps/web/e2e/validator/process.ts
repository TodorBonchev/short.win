/**
 * Background processes of an E2E run (validator, app preview server, optional indexer and
 * Postgres): spawned detached with their output in `.e2e/logs/`, recorded in `.e2e/runtime.json`
 * so global teardown (or `node e2e/validator/stop.ts`) stops them, and stopped by process group.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { PATHS } from '../env.ts';

export interface Runtime {
    validatorPid?: number;
    previewPid?: number;
    indexerPid?: number;
    /** `pg_ctl` data directory of the `full` profile's Postgres, when it was started. */
    pgData?: string;
    pgBin?: string;
    /** The seed's protocol ALT the app was built with (an app build is reused only for the same ALT). */
    appBuiltForAlt?: string | null;
    /** SHA-256 of the app's sources and build configuration the `.e2e/dist` build was made from. */
    appBuiltFromSources?: string;
}

export function readRuntime(): Runtime {
    if (!existsSync(PATHS.runtime)) return {};
    try {
        return JSON.parse(readFileSync(PATHS.runtime, 'utf8')) as Runtime;
    } catch {
        return {};
    }
}

export function writeRuntime(r: Runtime): void {
    mkdirSync(PATHS.logs, { recursive: true });
    writeFileSync(PATHS.runtime, JSON.stringify(r, null, 2));
}

export function updateRuntime(patch: Partial<Runtime>): Runtime {
    const r = { ...readRuntime(), ...patch };
    writeRuntime(r);
    return r;
}

/** Spawns `cmd args` detached (its own process group), output appended to `.e2e/logs/<log>`. */
export function spawnLogged(
    cmd: string,
    args: readonly string[],
    log: string,
    opts: { cwd?: string; env?: NodeJS.ProcessEnv } = {},
): ChildProcess {
    mkdirSync(PATHS.logs, { recursive: true });
    const fd = openSync(join(PATHS.logs, log), 'a');
    const child = spawn(cmd, [...args], {
        cwd: opts.cwd,
        env: opts.env ?? process.env,
        detached: true,
        stdio: ['ignore', fd, fd],
    });
    closeSync(fd);
    child.unref();
    return child;
}

export function isAlive(pid: number | undefined): boolean {
    if (!pid) return false;
    try {
        process.kill(pid, 0);
        return true;
    } catch {
        return false;
    }
}

/** SIGTERM to the process group, SIGKILL after `graceMs`. */
export async function stopPid(pid: number | undefined, graceMs = 5_000): Promise<void> {
    if (!pid || !isAlive(pid)) return;
    const kill = (sig: NodeJS.Signals) => {
        try {
            process.kill(-pid, sig);
        } catch {
            try {
                process.kill(pid, sig);
            } catch {
                // gone
            }
        }
    };
    kill('SIGTERM');
    const deadline = Date.now() + graceMs;
    while (isAlive(pid) && Date.now() < deadline) await new Promise((r) => setTimeout(r, 100));
    if (isAlive(pid)) kill('SIGKILL');
}

/** Polls `probe` until it resolves true or `timeoutMs` passes. */
export async function waitFor(what: string, probe: () => Promise<boolean>, timeoutMs: number): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    let last: unknown = null;
    while (Date.now() < deadline) {
        try {
            if (await probe()) return;
        } catch (e) {
            last = e;
        }
        await new Promise((r) => setTimeout(r, 500));
    }
    throw new Error(`timed out waiting for ${what}${last ? `: ${String(last)}` : ''}`);
}
