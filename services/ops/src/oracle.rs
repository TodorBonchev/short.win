//! Preflight the account named by an OracleFeed proposal. Governance still reviews which
//! asset the feed represents; a matching feed id alone does not establish SOL/USD semantics.

use anyhow::{Context, ensure};
use shortwin_services_common::{AccountData, Chain};
use shortwin_venue_state::pyth;
use solana_pubkey::Pubkey;

pub fn validate_account(account: &AccountData, feed_id: &[u8; 32]) -> anyhow::Result<()> {
    ensure!(!account.executable, "oracle account must not be executable");
    let update = pyth::read(&account.owner.to_bytes(), &account.data)
        .map_err(|e| anyhow::anyhow!("oracle owner or PriceUpdateV2 layout: {e:?}"))?;
    ensure!(
        update.feed_id == *feed_id,
        "oracle account's feed id differs from the proposal"
    );
    Ok(())
}

pub async fn preflight<C: Chain>(chain: &C, key: Pubkey, feed_id: &[u8; 32]) -> anyhow::Result<()> {
    let accounts = chain
        .accounts(&[key])
        .await
        .context("reading proposed oracle account")?;
    let account = accounts
        .first()
        .and_then(Option::as_ref)
        .with_context(|| format!("oracle account {key} does not exist"))?;
    validate_account(account, feed_id).with_context(|| format!("oracle account {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposed_feed_must_match_a_receiver_owned_price_update() {
        let mut account = AccountData {
            lamports: 1,
            owner: Pubkey::new_from_array(pyth::RECEIVER),
            executable: false,
            data: include_bytes!("../../../tests/fixtures/accounts/pyth_sol_usd_shard0.bin")
                .to_vec(),
        };
        let feed = pyth::read(&pyth::RECEIVER, &account.data).unwrap().feed_id;
        validate_account(&account, &feed).unwrap();
        assert!(validate_account(&account, &[0; 32]).is_err());
        account.owner = Pubkey::new_unique();
        assert!(validate_account(&account, &feed).is_err());
        account.owner = Pubkey::new_from_array(pyth::RECEIVER);
        account.executable = true;
        assert!(validate_account(&account, &feed).is_err());
        account.executable = false;
        account.data[0] ^= 1;
        assert!(validate_account(&account, &feed).is_err());
        account.data.truncate(8);
        assert!(validate_account(&account, &feed).is_err());
    }
}
