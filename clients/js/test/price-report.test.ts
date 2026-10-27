/** Borsh boundary vectors independent of the generated encoders and their decoders. */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { AccountRole, address, createNoopSigner, getAddressEncoder } from '@solana/kit';
import * as client from '../src/index.ts';

const key = address('So11111111111111111111111111111111111111112');
const publisher = createNoopSigner(address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE'));
const bytes = (n: number) => new Uint8Array(32).fill(n);
const disc = (name: string) => createHash('sha256').update(name).digest().subarray(0, 8);
const u64 = (n: bigint) => {
    const b = Buffer.alloc(8);
    b.writeBigUInt64LE(n);
    return b;
};
const i64 = (n: bigint) => {
    const b = Buffer.alloc(8);
    b.writeBigInt64LE(n);
    return b;
};

test('report payload matches the Rust field order, signed Clock times and Borsh status tags', async () => {
    const domain = {
        genesisHash: bytes(1), program: key, config: key, market: key, baseMint: key,
        baseTokenProgram: key, quoteMint: key, quoteTokenProgram: key, baseDecimals: 6, quoteDecimals: 9,
    };
    const model = { version: 1, epoch: 2n, policyHash: bytes(3) };
    const input = {
        version: 1, domain, quantity: (1n << 64n) - 1n, model, configurationEpoch: 4n,
        publisher: publisher.address, sequence: 5n, startSlot: 6n, startTs: -7n,
        endSlot: 8n, endTs: -9n, observationDigest: bytes(10),
    };
    const keyBytes = Buffer.from(getAddressEncoder().encode(key));
    const expectedPrefix = Buffer.concat([
        disc('global:publish_price_report'), Buffer.from([1, 0]), bytes(1),
        ...Array.from({ length: 7 }, () => keyBytes), Buffer.from([6, 9]), u64(input.quantity),
        Buffer.from([1, 0, 0, 0]), u64(2n), bytes(3), u64(4n),
        Buffer.from(getAddressEncoder().encode(publisher.address)), u64(5n), u64(6n), i64(-7n),
        u64(8n), i64(-9n), bytes(10),
    ]);
    for (const status of [client.priceReportStatusArgs('Unknown'), client.priceReportStatusArgs('Finite', {
        lower: 11n, central: 12n, upper: 13n,
    })]) {
        const expected = Buffer.concat([expectedPrefix, status.__kind === 'Unknown'
            ? Buffer.from([0]) : Buffer.concat([Buffer.from([1]), u64(11n), u64(12n), u64(13n)])]);
        const encoded = client.getPublishPriceReportInstructionDataEncoder().encode({ ...input, status });
        assert.deepEqual(Buffer.from(encoded), expected);
        assert.equal(encoded.length, status.__kind === 'Unknown' ? 433 : 457);
        const ix = await client.getPublishPriceReportInstructionAsync({
            ...input, publisher, publisherArg: publisher.address, config: key, market: key, priceReport: key, status,
        });
        assert.deepEqual(Buffer.from(ix.data), expected);
        assert.deepEqual(ix.accounts.map((a) => a.role), [
            AccountRole.READONLY_SIGNER, AccountRole.READONLY, AccountRole.READONLY, AccountRole.WRITABLE,
        ]);
        assert.equal(client.parsePublishPriceReportInstruction(ix).data.publisher, publisher.address);
    }
});

test('report account is exactly 1547 bytes; token repayment retains its explicit u64 rent bound', () => {
    const stateBytes = Uint8Array.from({ length: 1488 }, (_, i) => i % 251);
    const encoded = client.getPriceReportEncoder().encode({
        version: 1, bump: 254, market: key, quantity: 123n, modelEpoch: 456n, stateBytes,
    });
    const expected = Buffer.concat([
        disc('account:PriceReport'), Buffer.from([1, 0, 254]), Buffer.from(getAddressEncoder().encode(key)),
        u64(123n), u64(456n), stateBytes,
    ]);
    assert.equal(client.getPriceReportSize(), 1547);
    assert.deepEqual(Buffer.from(encoded), expected);
    assert.deepEqual(client.getPriceReportDecoder().decode(encoded).stateBytes, stateBytes);
    for (const maxRent of [0n, 1n, (1n << 64n) - 1n]) {
        assert.deepEqual(Buffer.from(client.getRepayShortInstructionDataEncoder().encode({ maxRent })),
            Buffer.concat([disc('global:repay_short'), u64(maxRent)]));
    }
    assert.equal(client.SHORTWIN_ERROR__REPAYMENT_RENT_ABOVE_MAX, 6111);
});
