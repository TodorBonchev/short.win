/**
 * The two strings of pump's v3 adapter (pump-final §17 D4; FRONTEND.md §4.3, the owner file's
 * section 9): the program's `CurveCashbackCoin` (6109) and `CurveStraddle` (6031) messages, which
 * this app shows where the program's refusal does (`errors.ts`: the review's error and Create
 * market's listing refusal). Approved as drafted by the owner on 2026-10-13 (D4), byte for byte: the
 * program's own `#[msg]` text (`programs/shortwin/src/errors.rs`), with its straight apostrophe and,
 * for `CurveStraddle`, no final full stop, unlike the rest of the error table. A change needs the
 * owner's approval, the program, this code and FRONTEND §4.3 together (`copy.test.ts` holds the
 * three to each other).
 *
 * The close form's straddle notice and its string are gone with it (D4): a close past a live
 * curve's `rT` completes the curve by pump's synthetic migration (FE-3 closed).
 */
import { type CopyEntry, c } from './copyNav.ts';

export const PUMP_COPY = {
    'error.curveCashbackCoin': c(
        "pump.fun's current trades do not support cashback coins on the curve. This coin can be shorted once it graduates to PumpSwap.",
        'Create market: the listing refusal of a live cashback curve, above its code (`create-check-reason-CurveCashbackCoin`); the review’s error when the program refuses a transaction with 6109 `CurveCashbackCoin` (src/config/errors.ts)',
        'pump-final §6.3, owner D1 (a): pump’s v3 trades refuse a cashback coin (pump 6094 CashbackCoinNotSupported), so a live cashback curve is refused (programs/shortwin/src/venues/detect.rs classify_pump_with; venues/detect.ts classifyPump); a complete one classifies as PumpMigrating / PumpSwap and lists (C2). pump’s create_v2 no longer creates cashback coins (pump-public-docs PUMP_CASHBACK_README).',
    ),
    'error.curveStraddle': c(
        'Buyback exceeds what the curve and its post-migration pool can deliver',
        'The review’s error when the program refuses a close with 6031 `CurveStraddle` (src/config/errors.ts): a buyback past what the live curve and its would-be pool hold, which the per-market cap keeps out of reach (pump-final C8)',
        'programs/shortwin/src/venues/pump_curve.rs close_venue_checks: past rT a close needs the synthetic route and q − rT < X_s = curve_base_ata − rT (pump’s NotEnoughTokensToBuy); pump-final §5.2, §6.1.7.',
    ),
} as const satisfies Record<string, CopyEntry>;

export type PumpCopyId = keyof typeof PUMP_COPY;
