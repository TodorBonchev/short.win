//! The JSON-RPC backend of [`Chain`] (a real cluster; ARCHITECTURE §7.1 "RPC websocket
//! fallback" — the Yellowstone account stream of §7.1 "Inputs" is a later milestone, so v0
//! polls `getMultipleAccounts` / `getProgramAccounts` each tick).
//!
//! - Reads use the configured commitment (default `confirmed`).
//! - `simulate` asks for inner instructions (event decoding) and replaces nothing: the keeper
//!   simulates the exact transaction it will send (signatures are not verified).
//! - `send` submits with `skipPreflight` (the keeper simulated already), then polls the
//!   signature until it is confirmed, fails, or `confirm_timeout` passes, and fetches the landed
//!   transaction's meta (logs, fee, compute units, inner instructions).
//! - A cluster that refuses a transaction before running it (e.g. a v1 transaction on a
//!   cluster without SIMD-0385, an oversized transaction) is reported as
//!   [`TxError::Rejected`], which the keeper's v1 → v0 fallback keys on.

use std::time::Duration;

use anyhow::{Context, anyhow};
use solana_account_decoder_client_types::UiAccountEncoding;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::client_error::{Error as ClientError, ErrorKind};
use solana_rpc_client_api::config::{
    RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcSendTransactionConfig,
    RpcSimulateTransactionConfig, RpcTransactionConfig,
};
use solana_rpc_client_api::filter::{Memcmp as RpcMemcmp, RpcFilterType};
use solana_rpc_client_api::request::RpcError;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction_status_client_types::{
    UiInnerInstructions, UiInstruction, UiTransactionEncoding,
};

use crate::chain::{AccountData, Chain, ChainClock, InnerIx, Memcmp, TxError, TxOutcome};

/// Largest `getMultipleAccounts` request.
const MULTIPLE_ACCOUNTS_MAX: usize = 100;

/// A JSON-RPC cluster (module docs).
pub struct RpcChain {
    client: RpcClient,
    commitment: CommitmentConfig,
    confirm_timeout: Duration,
    poll_interval: Duration,
}

impl RpcChain {
    /// A client for `url` at `confirmed` commitment, waiting up to 8 s for a confirmation.
    pub fn new(url: impl Into<String>) -> Self {
        let commitment = CommitmentConfig::confirmed();
        Self {
            client: RpcClient::new_with_commitment(url.into(), commitment),
            commitment,
            // About 20 slots: a keeper transaction that has not landed by then is retried
            // with an escalated priority price next tick rather than waited for.
            confirm_timeout: Duration::from_secs(8),
            poll_interval: Duration::from_millis(400),
        }
    }

    /// Overrides the confirmation timeout.
    pub fn with_confirm_timeout(mut self, timeout: Duration) -> Self {
        self.confirm_timeout = timeout;
        self
    }

    /// The underlying client.
    pub fn client(&self) -> &RpcClient {
        &self.client
    }

    fn account_of(ui: solana_account_decoder_client_types::UiAccount) -> Option<AccountData> {
        let data = ui.data.decode()?;
        let owner = ui.owner.parse().ok()?;
        Some(AccountData {
            lamports: ui.lamports,
            owner,
            executable: ui.executable,
            data,
        })
    }
}

/// `true` if an RPC error means the cluster refused the transaction before running it.
fn is_rejection(e: &ClientError) -> Option<String> {
    match e.kind() {
        ErrorKind::RpcError(RpcError::RpcResponseError { code, message, .. }) => {
            // -32602 invalid params (unsupported version, too large, undecodable),
            // -32003 signature verification, -32013 transaction length.
            matches!(code, -32602 | -32003 | -32013).then(|| message.clone())
        }
        _ => None,
    }
}

fn inner_of(
    inner: Option<Vec<UiInnerInstructions>>,
    keys: &[Pubkey],
    n_top: usize,
) -> Vec<Vec<InnerIx>> {
    let mut out = vec![Vec::new(); n_top];
    for group in inner.unwrap_or_default() {
        let Some(slot) = out.get_mut(usize::from(group.index)) else {
            continue;
        };
        for ix in group.instructions {
            if let UiInstruction::Compiled(c) = ix {
                let Some(program_id) = keys.get(usize::from(c.program_id_index)).copied() else {
                    continue;
                };
                let Ok(data) = bs58::decode(&c.data).into_vec() else {
                    continue;
                };
                slot.push(InnerIx { program_id, data });
            }
        }
    }
    out
}

impl Chain for RpcChain {
    async fn clock(&self) -> anyhow::Result<ChainClock> {
        let slot = self
            .client
            .get_slot_with_commitment(self.commitment)
            .await
            .context("getSlot")?;
        let clock = self
            .client
            .get_account_with_commitment(&solana_sdk_ids_clock(), self.commitment)
            .await
            .context("Clock sysvar")?
            .value
            .context("no Clock sysvar")?;
        // Clock sysvar: slot u64, epoch_start_timestamp i64, epoch u64, leader_schedule_epoch
        // u64, unix_timestamp i64.
        let ts = clock
            .data
            .get(32..40)
            .map(|b| i64::from_le_bytes(b.try_into().unwrap_or([0; 8])))
            .context("short Clock sysvar")?;
        Ok(ChainClock {
            slot,
            unix_timestamp: ts,
        })
    }

    async fn accounts(&self, keys: &[Pubkey]) -> anyhow::Result<Vec<Option<AccountData>>> {
        let mut out = Vec::with_capacity(keys.len());
        for chunk in keys.chunks(MULTIPLE_ACCOUNTS_MAX) {
            let res = self
                .client
                .get_multiple_accounts_with_commitment(chunk, self.commitment)
                .await
                .context("getMultipleAccounts")?;
            out.extend(res.value.into_iter().map(|a| {
                a.map(|a| AccountData {
                    lamports: a.lamports,
                    owner: a.owner,
                    executable: a.executable,
                    data: a.data,
                })
            }));
        }
        Ok(out)
    }

    async fn program_accounts(
        &self,
        program: &Pubkey,
        filters: &[Memcmp],
    ) -> anyhow::Result<Vec<(Pubkey, AccountData)>> {
        let config = RpcProgramAccountsConfig {
            filters: Some(
                filters
                    .iter()
                    .map(|f| {
                        RpcFilterType::Memcmp(RpcMemcmp::new_base58_encoded(f.offset, &f.bytes))
                    })
                    .collect(),
            ),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(self.commitment),
                ..RpcAccountInfoConfig::default()
            },
            with_context: None,
            sort_results: None,
        };
        let res = self
            .client
            .get_program_ui_accounts_with_config(program, config)
            .await
            .context("getProgramAccounts")?;
        Ok(res
            .into_iter()
            .filter_map(|(k, a)| Self::account_of(a).map(|a| (k, a)))
            .collect())
    }

    async fn latest_blockhash(&self) -> anyhow::Result<Hash> {
        self.client
            .get_latest_blockhash()
            .await
            .context("getLatestBlockhash")
    }

    async fn simulate(&self, tx: &VersionedTransaction) -> anyhow::Result<TxOutcome> {
        let config = RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: false,
            commitment: Some(self.commitment),
            encoding: Some(UiTransactionEncoding::Base64),
            accounts: None,
            min_context_slot: None,
            inner_instructions: true,
        };
        let res = match self
            .client
            .simulate_transaction_with_config(tx, config)
            .await
        {
            Ok(r) => r.value,
            Err(e) => {
                if let Some(msg) = is_rejection(&e) {
                    return Ok(TxOutcome {
                        err: Some(TxError::Rejected(msg)),
                        ..TxOutcome::default()
                    });
                }
                return Err(anyhow!(e).context("simulateTransaction"));
            }
        };
        let keys = tx.message.static_account_keys().to_vec();
        let n_top = tx.message.instructions().len();
        Ok(TxOutcome {
            signature: Default::default(),
            err: res.err.map(|e| TxError::from_transaction_error(e.into())),
            logs: res.logs.unwrap_or_default(),
            units_consumed: res.units_consumed.unwrap_or(0),
            fee: res.fee.unwrap_or(0),
            inner: inner_of(res.inner_instructions, &keys, n_top),
        })
    }

    async fn send(&self, tx: &VersionedTransaction) -> anyhow::Result<TxOutcome> {
        let config = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(self.commitment.commitment),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(0),
            min_context_slot: None,
        };
        let signature = match self.client.send_transaction_with_config(tx, config).await {
            Ok(s) => s,
            Err(e) => {
                if let Some(msg) = is_rejection(&e) {
                    return Ok(TxOutcome {
                        err: Some(TxError::Rejected(msg)),
                        ..TxOutcome::default()
                    });
                }
                return Err(anyhow!(e).context("sendTransaction"));
            }
        };
        let deadline = tokio::time::Instant::now() + self.confirm_timeout;
        let status = loop {
            let st = self
                .client
                .get_signature_statuses(&[signature])
                .await
                .context("getSignatureStatuses")?;
            if let Some(Some(s)) = st.value.into_iter().next()
                && s.satisfies_commitment(self.commitment)
            {
                break s;
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(TxOutcome {
                    signature,
                    err: Some(TxError::Dropped("not confirmed before the timeout".into())),
                    ..TxOutcome::default()
                });
            }
            tokio::time::sleep(self.poll_interval).await;
        };
        let mut outcome = TxOutcome {
            signature,
            err: status.err.map(TxError::from_transaction_error),
            ..TxOutcome::default()
        };
        let cfg = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Base64),
            commitment: Some(self.commitment),
            max_supported_transaction_version: Some(1),
        };
        if let Ok(landed) = self
            .client
            .get_transaction_with_config(&signature, cfg)
            .await
            && let Some(meta) = landed.transaction.meta
        {
            let keys = tx.message.static_account_keys().to_vec();
            let n_top = tx.message.instructions().len();
            outcome.fee = meta.fee;
            outcome.logs = Option::<Vec<String>>::from(meta.log_messages).unwrap_or_default();
            outcome.units_consumed = Option::<u64>::from(meta.compute_units_consumed).unwrap_or(0);
            outcome.inner = inner_of(
                Option::<Vec<UiInnerInstructions>>::from(meta.inner_instructions),
                &keys,
                n_top,
            );
        }
        Ok(outcome)
    }

    async fn recent_priority_fees(&self, writable: &[Pubkey]) -> anyhow::Result<Vec<u64>> {
        let fees = self
            .client
            .get_recent_prioritization_fees(writable)
            .await
            .context("getRecentPrioritizationFees")?;
        Ok(fees.into_iter().map(|f| f.prioritization_fee).collect())
    }
}

/// The Clock sysvar's address.
fn solana_sdk_ids_clock() -> Pubkey {
    solana_pubkey::pubkey!("SysvarC1ock11111111111111111111111111111111")
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_transaction_status_client_types::UiCompiledInstruction;

    #[test]
    fn inner_instructions_resolve_programs_and_decode_data() {
        let keys = [Pubkey::new_unique(), Pubkey::new_unique()];
        let ui = vec![UiInnerInstructions {
            index: 1,
            instructions: vec![UiInstruction::Compiled(UiCompiledInstruction {
                program_id_index: 1,
                accounts: vec![0],
                data: bs58::encode([1u8, 2, 3]).into_string(),
                stack_height: Some(2),
            })],
        }];
        let inner = inner_of(Some(ui), &keys, 3);
        assert_eq!(inner.len(), 3);
        assert!(inner[0].is_empty() && inner[2].is_empty());
        assert_eq!(
            inner[1],
            vec![InnerIx {
                program_id: keys[1],
                data: vec![1, 2, 3]
            }]
        );
        assert_eq!(inner_of(None, &keys, 1), vec![Vec::new()]);
    }
}
