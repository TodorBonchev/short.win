/**
 * Profile `full` (FRONTEND.md §10.2, local and optional): the real `shortwin-indexer` against the
 * E2E validator and a user-space PostgreSQL 17 — the zonky binaries `postgresql_embedded` already
 * caches for the indexer's tests (INDEXER §11.1; `~/.cache/shortwin/postgresql-zonky/<version>`,
 * or `$SHORTWIN_PG_INSTALL_DIR`). No Docker, no root, no system Postgres. The data directory is
 * `.e2e/pg` (on disk), the server listens on 127.0.0.1:15432, the API on 127.0.0.1:18080.
 *
 * Needs `SHORTWIN_INDEXER_BIN` (e.g. `services/target/release/shortwin-indexer`, built with
 * `cargo build --manifest-path services/Cargo.toml -p shortwin-indexer --release`).
 */
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { E2E_DIR, PATHS, PORTS, URLS } from '../env.ts';
import { PROGRAMS } from './chain.ts';
import { isAlive, readRuntime, spawnLogged, stopPid, updateRuntime, waitFor } from './process.ts';

async function indexerUp(): Promise<boolean> {
    try {
        return (await fetch(`${URLS.indexer}/v1/status`, { signal: AbortSignal.timeout(2_000) })).ok;
    } catch {
        return false;
    }
}

function pgBin(): string {
    const env = process.env.SHORTWIN_PG_INSTALL_DIR;
    const base = env ?? join(homedir(), '.cache', 'shortwin', 'postgresql-zonky');
    if (existsSync(join(base, 'bin', 'pg_ctl'))) return join(base, 'bin');
    const versions = existsSync(base) ? readdirSync(base).sort().reverse() : [];
    for (const v of versions) if (existsSync(join(base, v, 'bin', 'pg_ctl'))) return join(base, v, 'bin');
    throw new Error(
        `no PostgreSQL binaries under ${base}: run the indexer's tests once (they download them) or set SHORTWIN_PG_INSTALL_DIR`,
    );
}

function run(cmd: string, args: string[], what: string): void {
    const r = spawnSync(cmd, args, { stdio: 'inherit' });
    if (r.status !== 0) throw new Error(`${what} failed (exit ${r.status ?? r.signal})`);
}

export async function startIndexer(): Promise<void> {
    if (isAlive(readRuntime().indexerPid) && (await indexerUp())) {
        console.log(`[e2e] reusing shortwin-indexer at ${URLS.indexer}`);
        return;
    }
    const bin = process.env.SHORTWIN_INDEXER_BIN;
    if (!bin || !existsSync(bin)) {
        throw new Error('SHORTWIN_E2E_PROFILE=full needs SHORTWIN_INDEXER_BIN (a built shortwin-indexer)');
    }
    const pg = pgBin();
    const data = join(PATHS.pg, 'data');
    const sock = join(PATHS.pg, 'sock');
    mkdirSync(sock, { recursive: true });
    if (!existsSync(join(data, 'PG_VERSION'))) {
        run(join(pg, 'initdb'), ['-D', data, '-U', 'shortwin', '--auth', 'trust', '-E', 'UTF8'], 'initdb');
    }
    run(
        join(pg, 'pg_ctl'),
        [
            '-D',
            data,
            '-l',
            join(PATHS.logs, 'postgres.log'),
            '-o',
            `-p ${PORTS.postgres} -k ${sock} -c listen_addresses=127.0.0.1`,
            '-w',
            'start',
        ],
        'pg_ctl start',
    );
    updateRuntime({ pgData: data, pgBin: pg });
    const config = join(E2E_DIR, 'indexer.toml');
    writeFileSync(
        config,
        [
            '[database]',
            `url = "postgres://shortwin@127.0.0.1:${PORTS.postgres}/postgres"`,
            'migrate_on_start = false',
            '[ingest]',
            `rpc_url = "${URLS.rpc}"`,
            `ws_url = "${URLS.ws}"`,
            `program_id = "${PROGRAMS.shortwin}"`,
            'live_source = "logs_subscribe"',
            '[api]',
            `listen = "127.0.0.1:${PORTS.indexer}"`,
            'cors_origins = ["*"]',
            '',
        ].join('\n'),
    );
    run(bin, ['--config', config, 'migrate'], 'shortwin-indexer migrate');
    run(bin, ['--config', config, 'backfill'], 'shortwin-indexer backfill');
    const child = spawnLogged(bin, ['--config', config, 'run'], 'indexer.log');
    updateRuntime({ indexerPid: child.pid });
    await waitFor('shortwin-indexer (see .e2e/logs/indexer.log)', indexerUp, 120_000);
    console.log(`[e2e] shortwin-indexer at ${URLS.indexer} (Postgres ${pg})`);
}

export async function stopIndexer(): Promise<void> {
    const r = readRuntime();
    await stopPid(r.indexerPid);
    if (r.pgData && r.pgBin) {
        spawnSync(join(r.pgBin, 'pg_ctl'), ['-D', r.pgData, '-m', 'fast', 'stop'], { stdio: 'ignore' });
    }
    updateRuntime({ indexerPid: undefined, pgData: undefined, pgBin: undefined });
}
