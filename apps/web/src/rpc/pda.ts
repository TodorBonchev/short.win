/**
 * short.win PDAs the generated client does not derive. Every short.win instruction is
 * `#[event_cpi]`, so every builder names the event authority `PDA(short.win, ["__event_authority"])`;
 * one derivation here serves every plan (create market, lend, short, the post-wipe sweep).
 */
import { type Address, getProgramDerivedAddress } from '@solana/kit';
import { SHORTWIN_PROGRAM } from '../config/programs.ts';

let cached: Promise<Address> | null = null;

/** Anchor's `emit_cpi!` authority of short.win: `PDA(short.win, ["__event_authority"])`. */
export function eventAuthority(): Promise<Address> {
    cached ??= getProgramDerivedAddress({ programAddress: SHORTWIN_PROGRAM, seeds: ['__event_authority'] }).then(
        ([pda]) => pda,
    );
    return cached;
}
