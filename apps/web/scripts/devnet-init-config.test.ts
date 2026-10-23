import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { address } from '@solana/kit';
import { parseArgs, readPyth } from './devnet-init-config.ts';

const keys = [
    '--upgrade-authority',
    '3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14',
    '--governance',
    '2DckMbjuXP27UCFoPZ3cURPz8Y8RiqFHJrrWAF8c3ybK',
    '--pause-authority',
    'GHcdeRUzPMLtCPdCoZ3NuGsJ422NmKK1WjRUpXe3a6mu',
    '--coverage-authority',
    '52oLAneAy9CUhtAnSjfjxMX7fqYnbCuG4Nd9hKvmKQpS',
    '--fee-recipient',
    'HEjqYyDMp3UtT2sQWiqpNLd9t4w3svDXFDLfZ6bs3XSY',
];

describe('devnet initialization command boundary', () => {
    it('defaults to unsigned, venue-disabled planning without a keypair path', () => {
        const a = parseArgs(keys, {});
        assert.equal(a.send, false);
        assert.equal(a.venueBits, 0);
        assert.equal(a.authorityKeypair, undefined);
        assert.equal(a.maxDebitLamports, 200_000_000n);
    });

    it('requires a reviewed plan hash and a keypair path before send mode', () => {
        assert.throws(() => parseArgs([...keys, '--send'], {}), /requires --expected-plan-sha256/);
        const a = parseArgs(
            [
                ...keys,
                '--send',
                '--expected-plan-sha256',
                'a'.repeat(64),
                '--authority-keypair',
                '/outside-repo/authority.json',
            ],
            {},
        );
        assert.equal(a.send, true);
        assert.equal(a.expectedPlanSha256, 'a'.repeat(64));
        assert.equal(a.authorityKeypair, '/outside-repo/authority.json');
    });

    it('rejects unsupported venue bits, excess debit ceilings, and duplicate authorities', () => {
        assert.throws(() => parseArgs([...keys, '--venue-bits', '4'], {}), /only Pump\/PumpSwap/);
        assert.throws(() => parseArgs([...keys, '--max-debit-lamports', '2000000000'], {}), /within 150000000/);
        const duplicate = [...keys];
        duplicate[duplicate.indexOf('--governance') + 1] = duplicate[duplicate.indexOf('--pause-authority') + 1];
        assert.throws(() => parseArgs(duplicate, {}), /must be distinct/);
    });
});

function pythAccount(publishTime = 950n) {
    const bytes = Buffer.alloc(134);
    Buffer.from('22f123639d7ef4cd', 'hex').copy(bytes, 0);
    bytes[40] = 1; // VerificationLevel::Full
    Buffer.from('ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d', 'hex').copy(bytes, 41);
    bytes.writeBigInt64LE(10_000_000_000n, 73);
    bytes.writeBigUInt64LE(100_000n, 81);
    bytes.writeInt32LE(-8, 89);
    bytes.writeBigInt64LE(publishTime, 93);
    return {
        owner: address('rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ'),
        executable: false,
        lamports: 1n,
        data: [bytes.toString('base64'), 'base64'] as const,
    };
}

describe('devnet initialization oracle preflight', () => {
    it('accepts the exact full SOL/USD feed within the protocol age/confidence bounds', () => {
        const update = readPyth(pythAccount(), 1_000n, 120, 100);
        assert.equal(update.ageS, '50');
        assert.equal(update.exponent, -8);
    });

    it('rejects a stale or wrong-feed update before any transaction is built', () => {
        assert.throws(() => readPyth(pythAccount(800n), 1_000n, 120, 100), /outside ±120s/);
        const account = pythAccount();
        const bytes = Buffer.from(account.data[0], 'base64');
        bytes[41] ^= 1;
        assert.throws(
            () => readPyth({ ...account, data: [bytes.toString('base64'), 'base64'] }, 1_000n, 120, 100),
            /wrong SOL\/USD feed/,
        );
    });
});
