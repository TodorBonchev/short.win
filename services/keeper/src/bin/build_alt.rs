//! `shortwin-build-alt`: the protocol address lookup table (ARCHITECTURE §6.4, A28;
//! `shortwin_keeper::alt`).
//!
//! ```text
//! shortwin-build-alt print  [--rpc <url>] [--static-only]
//! shortwin-build-alt create --rpc <url> --keypair <payer> [--authority <keypair>] [--freeze]
//!                           [--priority-price <µlamports/CU>] [--static-only]
//! shortwin-build-alt extend --rpc <url> --keypair <payer> --table <alt> [--authority <keypair>]
//!                           [--market <market> [--protocol-table <protocol alt>]]
//!                           [--priority-price <µlamports/CU>] [--static-only]
//! shortwin-build-alt check  --rpc <url> --table <alt> [--static-only]
//!                           [--market <market> [--protocol-table <protocol alt>]]
//! shortwin-build-alt market --rpc <url> --market <market> [--protocol-table <protocol alt>]
//!                           [--keypair <payer> [--authority <keypair>] [--priority-price <µlamports/CU>]]
//! ```
//!
//! - `print`: the address list, one `index key label` line each. Without `--rpc` (or with
//!   `--static-only`) only the static keys; with `--rpc` also the live pump / PumpSwap fee and
//!   buyback recipients and their WSOL ATAs (read from `Global` and `GlobalConfig`).
//! - `create`: a new table owned by `--authority` (default: the payer) at the latest finalized
//!   slot, extended with the whole list; `--freeze` removes the authority afterwards (never
//!   deactivated, never extended again). Prints the table's address: put it in the keeper's
//!   `lookup_tables`.
//! - `extend`: adds whatever the list has and the table lacks (e.g. a new pump recipient).
//!   With `--market`, refreshes that market's keys, including pool switches and new extras.
//! - `check`: exit 0 if the table holds every address of the list and is active, 1 otherwise
//!   (the missing ones are printed); also reports extra entries and the authority.
//!
//! - `market` (M7, OWNER-M7-1's fallback; ARCH §6.4): a market's own table
//!   (`alt::market_entries`) — the per-market keys its trading transactions and cranks name that
//!   the protocol table (`--protocol-table`, with `--table` retained as an alias for this command;
//!   else the list `create` would build) leaves inline. Prints the
//!   list and its rent; with `--keypair` also creates it, extendable (never frozen: a new extras
//!   set or a pool switch adds keys; closing it after deactivation returns the rent). Put its
//!   address in the keeper's `lookup_tables` and publish it to clients (the PumpSwap open with the
//!   largest extras set, and v0 crank batches of more than 3 PumpSwap markets, use it).
//!
//! Every transaction is simulated first; compute units `⌈simulated × 1.1⌉`.

use std::str::FromStr;

use anyhow::{Context, bail};
use shortwin_keeper::alt::{self, AltEntry};
use shortwin_services_common::rpc::RpcChain;
use solana_keypair::{Keypair, read_keypair_file};
use solana_pubkey::Pubkey;
use solana_signer::Signer;

const USAGE: &str = "usage: shortwin-build-alt <print|create|extend|check|market> [--rpc <url>] \
     [--keypair <payer>] [--authority <keypair>] [--table <alt>] [--market <market>] \
     [--protocol-table <protocol alt>] [--freeze] \
     [--priority-price <µlamports/CU>] [--static-only]";

/// A finalized slot returned by one public RPC backend can still be ahead of the bank that
/// simulates or forwards the create transaction. Keep a margin within SlotHashes' 512 entries.
const RECENT_SLOT_MARGIN: u64 = 128;

fn alt_recent_slot(finalized_slot: u64) -> u64 {
    finalized_slot.saturating_sub(RECENT_SLOT_MARGIN)
}

#[derive(Default)]
struct Args {
    cmd: String,
    rpc: Option<String>,
    keypair: Option<String>,
    authority: Option<String>,
    table: Option<Pubkey>,
    market: Option<Pubkey>,
    protocol_table: Option<Pubkey>,
    freeze: bool,
    price: u64,
    static_only: bool,
}

fn parse_args() -> anyhow::Result<Args> {
    parse_args_from(std::env::args().skip(1))
}

fn parse_args_from(args: impl IntoIterator<Item = String>) -> anyhow::Result<Args> {
    let mut a = Args::default();
    let mut it = args.into_iter();
    a.cmd = it.next().context(USAGE)?;
    while let Some(flag) = it.next() {
        let mut val = || {
            it.next()
                .with_context(|| format!("{flag} needs a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--rpc" => a.rpc = Some(val()?),
            "--keypair" => a.keypair = Some(val()?),
            "--authority" => a.authority = Some(val()?),
            "--table" => a.table = Some(Pubkey::from_str(&val()?).context("--table")?),
            "--market" => a.market = Some(Pubkey::from_str(&val()?).context("--market")?),
            "--protocol-table" => {
                a.protocol_table = Some(Pubkey::from_str(&val()?).context("--protocol-table")?)
            }
            "--priority-price" => a.price = val()?.parse().context("--priority-price")?,
            "--freeze" => a.freeze = true,
            "--static-only" => a.static_only = true,
            "-h" | "--help" => bail!("{USAGE}"),
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    if a.market.is_some() && !matches!(a.cmd.as_str(), "market" | "extend" | "check") {
        bail!("--market is supported by market, extend and check");
    }
    if a.protocol_table.is_some() && a.market.is_none() {
        bail!("--protocol-table requires --market");
    }
    if a.cmd == "market" && a.table.is_some() && a.protocol_table.is_some() {
        bail!("use only --protocol-table (or its market-command alias --table)");
    }
    if a.cmd != "market" && a.table.is_some() && a.table == a.protocol_table {
        bail!("the market table and protocol table must be different");
    }
    if a.freeze && a.cmd != "create" {
        bail!("--freeze is supported only by create; market tables must remain extendable");
    }
    Ok(a)
}

fn keypair(path: &str) -> anyhow::Result<Keypair> {
    read_keypair_file(path).map_err(|e| anyhow::anyhow!("reading {path}: {e}"))
}

async fn entries(chain: Option<&RpcChain>, static_only: bool) -> anyhow::Result<Vec<AltEntry>> {
    match chain {
        Some(c) if !static_only => alt::live_entries(c).await,
        _ => alt::protocol_entries(alt::static_entries(), Vec::new()),
    }
}

fn print(entries: &[AltEntry]) {
    for (i, e) in entries.iter().enumerate() {
        println!("{i:>3} {} {}", e.key, e.label);
    }
    println!("# {} addresses (max {})", entries.len(), alt::MAX_ADDRESSES);
}

fn signers(a: &Args) -> anyhow::Result<(Keypair, Keypair)> {
    let payer = keypair(a.keypair.as_deref().context("--keypair is required")?)?;
    let authority = match &a.authority {
        Some(p) => keypair(p)?,
        None => payer.insecure_clone(),
    };
    Ok((payer, authority))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let a = parse_args()?;
    let chain = a.rpc.as_ref().map(|u| RpcChain::new(u.clone()));
    let need_rpc = || {
        chain
            .as_ref()
            .with_context(|| format!("--rpc is required\n{USAGE}"))
    };
    match a.cmd.as_str() {
        "print" => print(&entries(chain.as_ref(), a.static_only).await?),
        "create" => {
            let c = need_rpc()?;
            let (payer, authority) = signers(&a)?;
            let list = entries(Some(c), a.static_only).await?;
            let slot = alt_recent_slot(c.finalized_slot().await?);
            let table = alt::create(
                c,
                &payer,
                &authority,
                slot,
                &alt::keys(&list),
                a.freeze,
                a.price,
            )
            .await?;
            print(&list);
            let owner = if a.freeze {
                "frozen: no authority".to_string()
            } else {
                format!("authority {}", authority.pubkey())
            };
            println!("created {table} ({owner}); usable from the next slot");
        }
        "extend" => {
            let c = need_rpc()?;
            let table = a.table.context("--table is required")?;
            let (payer, authority) = signers(&a)?;
            let list = alt::selected_entries(c, a.market, a.protocol_table, a.static_only).await?;
            let n = alt::extend(c, &payer, &authority, &table, &alt::keys(&list), a.price).await?;
            println!("{table}: {n} addresses added");
        }
        "check" => {
            let c = need_rpc()?;
            let table = a.table.context("--table is required")?;
            let list = alt::selected_entries(c, a.market, a.protocol_table, a.static_only).await?;
            let state = alt::read_table(c, &table).await?;
            let expected = alt::keys(&list);
            let missing = state.missing(&expected);
            println!(
                "{table}: {} addresses, authority {}, {}",
                state.addresses.len(),
                state
                    .authority
                    .map_or_else(|| "none (frozen)".to_string(), |k| k.to_string()),
                if state.deactivation_slot == u64::MAX {
                    "active".to_string()
                } else {
                    format!("DEACTIVATED at slot {}", state.deactivation_slot)
                }
            );
            for k in state.addresses.iter().filter(|k| !expected.contains(k)) {
                println!("extra   {k}");
            }
            for k in &missing {
                let label = list
                    .iter()
                    .find(|e| e.key == *k)
                    .map_or("", |e| e.label.as_str());
                println!("MISSING {k} {label}");
            }
            if !missing.is_empty() || state.deactivation_slot != u64::MAX {
                std::process::exit(1);
            }
            println!("ok: every expected address is in the table");
        }
        "market" => {
            let c = need_rpc()?;
            let market = a.market.context("--market is required")?;
            let list =
                alt::selected_entries(c, Some(market), a.protocol_table.or(a.table), a.static_only)
                    .await?;
            print(&list);
            println!(
                "# rent {} lamports (recovered on close)",
                alt::table_rent_lamports(list.len())
            );
            if a.keypair.is_some() {
                let (payer, authority) = signers(&a)?;
                let slot = alt_recent_slot(c.finalized_slot().await?);
                let table = alt::create(
                    c,
                    &payer,
                    &authority,
                    slot,
                    &alt::keys(&list),
                    false,
                    a.price,
                )
                .await?;
                println!(
                    "created {table} for {market} (authority {}); usable from the next slot",
                    authority.pubkey()
                );
            }
        }
        other => bail!("unknown command {other}\n{USAGE}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> anyhow::Result<Args> {
        parse_args_from(values.iter().map(|v| (*v).to_string()))
    }

    #[test]
    fn market_refresh_keeps_destination_separate_from_protocol_table() {
        let market = Pubkey::new_unique().to_string();
        let table = Pubkey::new_unique().to_string();
        let protocol = Pubkey::new_unique().to_string();
        for command in ["extend", "check"] {
            let parsed = args(&[
                command,
                "--table",
                &table,
                "--market",
                &market,
                "--protocol-table",
                &protocol,
            ])
            .unwrap();
            assert_eq!(parsed.table.unwrap().to_string(), table);
            assert_eq!(parsed.protocol_table.unwrap().to_string(), protocol);
            assert_eq!(parsed.market.unwrap().to_string(), market);
            assert!(
                args(&[
                    command,
                    "--table",
                    &table,
                    "--market",
                    &market,
                    "--protocol-table",
                    &table
                ])
                .is_err()
            );
        }
        assert!(args(&["create", "--market", &market]).is_err());
        assert!(args(&["check", "--protocol-table", &protocol]).is_err());
        assert!(args(&["market", "--market", &market, "--freeze"]).is_err());
        assert!(args(&["market", "--market", &market, "--table", &protocol]).is_ok());
        assert!(
            args(&[
                "market",
                "--market",
                &market,
                "--table",
                &protocol,
                "--protocol-table",
                &protocol
            ])
            .is_err()
        );
    }

    #[test]
    fn recent_slot_keeps_a_margin_for_public_rpc_backend_skew() {
        assert_eq!(alt_recent_slot(512), 384);
        assert_eq!(alt_recent_slot(0), 0);
    }
}
