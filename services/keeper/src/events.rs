//! Decoding short.win events from a [`TxOutcome`] (simulated or landed; ARCHITECTURE
//! §3.10): `emit_cpi!` events from the inner instructions of short.win's self-CPI
//! (`EVENT_IX_TAG_LE ‖ discriminator ‖ Borsh`), and the `emit!` log events (`CoverageGap`,
//! an exit's `MintSafetyTripped`) from `Program data:` log lines. The pool-selection events
//! (`PoolProposed`, `PoolSwitched`, `PoolSwitchCancelled`; M4) are reported as [`PoolEvent`]s.

use anchor_lang::__private::base64::{self, Engine};
use anchor_lang::prelude::Pubkey;
use anchor_lang::{AnchorDeserialize, Discriminator, Event};
use shortwin_services_common::TxOutcome;

fn decode<T: Event + Discriminator + AnchorDeserialize>(payload: &[u8]) -> Option<T> {
    let body = payload.strip_prefix(T::DISCRIMINATOR)?;
    T::try_from_slice(body).ok()
}

/// Every `emit_cpi!` event of type `T` of top-level instruction `ix`.
pub fn of_ix<T: Event + Discriminator + AnchorDeserialize>(out: &TxOutcome, ix: usize) -> Vec<T> {
    let tag = anchor_lang::event::EVENT_IX_TAG_LE;
    out.inner
        .get(ix)
        .map(|inner| {
            inner
                .iter()
                .filter(|i| i.program_id == shortwin::ID)
                .filter_map(|i| i.data.strip_prefix(tag))
                .filter_map(decode::<T>)
                .collect()
        })
        .unwrap_or_default()
}

/// Every `emit_cpi!` event of type `T` in the transaction.
pub fn of<T: Event + Discriminator + AnchorDeserialize>(out: &TxOutcome) -> Vec<T> {
    (0..out.inner.len())
        .flat_map(|i| of_ix::<T>(out, i))
        .collect()
}

/// Every `emit!` log event of type `T` in the transaction's logs.
pub fn logged<T: Event + Discriminator + AnchorDeserialize>(out: &TxOutcome) -> Vec<T> {
    let engine = base64::engine::general_purpose::STANDARD;
    out.logs
        .iter()
        .filter_map(|line| line.strip_prefix("Program data: "))
        .filter_map(|b64| engine.decode(b64.trim()).ok())
        .filter_map(|p| decode::<T>(&p))
        .collect()
}

/// The program whose failure ended the transaction: the first `Program <id> failed` line
/// (the innermost failing program logs first).
pub fn failing_program(out: &TxOutcome) -> Option<anchor_lang::prelude::Pubkey> {
    out.logs.iter().find_map(|l| {
        let rest = l.strip_prefix("Program ")?;
        let (id, tail) = rest.split_once(' ')?;
        tail.starts_with("failed")
            .then(|| id.parse().ok())
            .flatten()
    })
}

/// Which pool-selection event (ARCH §3.10; M4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PoolEventKind {
    /// `PoolProposed`: a fast-path collection window opened or improved.
    Proposed,
    /// `PoolSwitched`: an adoption (listing, listing window, zero-position, fast path).
    Switched,
    /// `PoolSwitchCancelled`: a window closed without an adoption.
    Cancelled,
}

/// A pool-selection event (`PoolProposed`, `PoolSwitched`, `PoolSwitchCancelled` share their
/// fields; the Anchor event types derive neither `Debug` nor `Clone`, so the keeper reports
/// this copy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolEvent {
    pub what: PoolEventKind,
    pub market: Pubkey,
    pub slot: u64,
    pub ts: i64,
    /// The candidate pool (`Switched`: the pool now adopted).
    pub candidate: Pubkey,
    /// `shortwin::events::POOL_KIND_*` ([`pool_kind_name`]).
    pub kind: u8,
    /// The candidate's locked depth `D` (`Cancelled`: the best one seen).
    pub d_cand: u128,
    /// The incumbent's locked depth `pool_d` (0 for a list-only market).
    pub d_inc: u128,
    /// `Proposed` / `Cancelled`: the end of the collection window; `Switched`: the end of the
    /// listing window after the adoption (opens resume then).
    pub window_ends: i64,
}

macro_rules! pool_event_of {
    ($what:expr, $e:expr) => {
        PoolEvent {
            what: $what,
            market: $e.market,
            slot: $e.slot,
            ts: $e.ts,
            candidate: $e.candidate,
            kind: $e.kind,
            d_cand: $e.d_cand,
            d_inc: $e.d_inc,
            window_ends: $e.window_ends,
        }
    };
}

/// Every pool-selection event of the transaction, in instruction order.
pub fn pool_events(out: &TxOutcome) -> Vec<PoolEvent> {
    use shortwin::events::{PoolProposed, PoolSwitchCancelled, PoolSwitched};
    let mut v = Vec::new();
    for ix in 0..out.inner.len() {
        v.extend(
            of_ix::<PoolProposed>(out, ix)
                .into_iter()
                .map(|e| pool_event_of!(PoolEventKind::Proposed, e)),
        );
        v.extend(
            of_ix::<PoolSwitched>(out, ix)
                .into_iter()
                .map(|e| pool_event_of!(PoolEventKind::Switched, e)),
        );
        v.extend(
            of_ix::<PoolSwitchCancelled>(out, ix)
                .into_iter()
                .map(|e| pool_event_of!(PoolEventKind::Cancelled, e)),
        );
    }
    v
}

/// The name of a `POOL_KIND_*` value.
pub fn pool_kind_name(kind: u8) -> &'static str {
    use shortwin::events::{
        POOL_KIND_FAST, POOL_KIND_LISTING, POOL_KIND_LISTING_WINDOW, POOL_KIND_ZERO_POSITION,
    };
    match kind {
        POOL_KIND_LISTING => "listing",
        POOL_KIND_LISTING_WINDOW => "listing window",
        POOL_KIND_ZERO_POSITION => "zero-position",
        POOL_KIND_FAST => "fast path",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::{AnchorSerialize, Discriminator};
    use shortwin::events::{POOL_KIND_FAST, PoolSwitched};
    use shortwin_services_common::InnerIx;

    /// An `emit_cpi!` payload decodes into the keeper's copy with every field.
    #[test]
    fn pool_events_decode_from_the_self_cpi() {
        let e = PoolSwitched {
            market: Pubkey::new_unique(),
            slot: 7,
            ts: 1_700_000_000,
            candidate: Pubkey::new_unique(),
            kind: POOL_KIND_FAST,
            d_cand: 12_345,
            d_inc: 20_000,
            window_ends: 1_700_003_600,
        };
        let mut data = anchor_lang::event::EVENT_IX_TAG_LE.to_vec();
        data.extend_from_slice(PoolSwitched::DISCRIMINATOR);
        e.serialize(&mut data).unwrap();
        let out = TxOutcome {
            inner: vec![vec![
                InnerIx {
                    program_id: Pubkey::new_unique(),
                    data: data.clone(),
                },
                InnerIx {
                    program_id: shortwin::ID,
                    data,
                },
            ]],
            ..TxOutcome::default()
        };
        let got = pool_events(&out);
        assert_eq!(
            got,
            vec![PoolEvent {
                what: PoolEventKind::Switched,
                market: e.market,
                slot: 7,
                ts: e.ts,
                candidate: e.candidate,
                kind: POOL_KIND_FAST,
                d_cand: 12_345,
                d_inc: 20_000,
                window_ends: e.window_ends,
            }],
            "only short.win's own inner instruction counts"
        );
        assert_eq!(pool_kind_name(POOL_KIND_FAST), "fast path");
    }
}
