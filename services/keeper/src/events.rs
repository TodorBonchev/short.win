//! Decoding short.win events from a [`TxOutcome`] (simulated or landed; ARCHITECTURE
//! §3.10): `emit_cpi!` events from the inner instructions of short.win's self-CPI
//! (`EVENT_IX_TAG_LE ‖ discriminator ‖ Borsh`), and the `emit!` log events (`CoverageGap`,
//! an exit's `MintSafetyTripped`) from `Program data:` log lines.

use anchor_lang::__private::base64::{self, Engine};
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
