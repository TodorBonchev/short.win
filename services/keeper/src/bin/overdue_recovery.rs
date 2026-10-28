//! Read-only fixed-term collateral-default review; only explicit --send signs/sends.
//! Independent of the price service/reporter. No quote/price/no-route evidence is used.
//! One fixed instruction, max epoch-record rent signed on chain, bounded network fees,
//! native shared booking preview, exact-message simulation, one send, durable public receipts.
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{AccountDeserialize, Discriminator, InstructionData, ToAccountMetas};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shortwin::{
    constants::{CONFIG_VERSION, MARKET_VERSION},
    engine::quote_finance,
    state::{
        GlobalConfig, Market, MarketVault, QuoteFinanceExposure, QuoteFinanceMarket,
        QuoteFinancePosition, ShareEpoch,
    },
};
use shortwin_keeper::price_http::MAINNET_GENESIS;
use shortwin_services_common::{AccountData, Chain, chain::TxError, rpc::RpcChain, tx};
use shortwin_venue_state::program_data;
use solana_commitment_config::CommitmentConfig;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_keypair::Keypair;
use solana_message::{VersionedMessage, v0};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
    time::Duration,
};
const USAGE: &str = "usage: shortwin-overdue-recovery --config <file> --position <pubkey> --keeper <pubkey> [--send]";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Arguments,
    Configuration,
    PrivateFile,
    Chain,
    ProgramIdentity,
    Account,
    DefaultNotReady,
    Booking,
    RentCap,
    FeeCap,
    Funding,
    Simulation,
    Keypair,
    Receipt,
    ConfirmationUncertain,
    ConfirmedFailure,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "overdue recovery: {self:?}")
    }
}
struct Args {
    config: PathBuf,
    position: Pubkey,
    keeper: Pubkey,
    send: bool,
}
fn args(raw: impl IntoIterator<Item = OsString>) -> Result<Option<Args>, Error> {
    let raw: Vec<_> = raw.into_iter().take(9).collect();
    if raw.len() == 1 && matches!(raw[0].to_str(), Some("--help" | "-h")) {
        return Ok(None);
    }
    let (mut config, mut position, mut keeper, mut send) = (None, None, None, false);
    let mut iter = raw.into_iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--config") if config.is_none() => {
                config = Some(PathBuf::from(iter.next().ok_or(Error::Arguments)?))
            }
            Some("--position") if position.is_none() => {
                position = Some(
                    Pubkey::from_str(
                        iter.next()
                            .ok_or(Error::Arguments)?
                            .to_str()
                            .ok_or(Error::Arguments)?,
                    )
                    .map_err(|_| Error::Arguments)?,
                )
            }
            Some("--keeper") if keeper.is_none() => {
                keeper = Some(
                    Pubkey::from_str(
                        iter.next()
                            .ok_or(Error::Arguments)?
                            .to_str()
                            .ok_or(Error::Arguments)?,
                    )
                    .map_err(|_| Error::Arguments)?,
                )
            }
            Some("--send") if !send => send = true,
            _ => return Err(Error::Arguments),
        }
    }
    Ok(Some(Args {
        config: config.ok_or(Error::Arguments)?,
        position: position.ok_or(Error::Arguments)?,
        keeper: keeper.ok_or(Error::Arguments)?,
        send,
    }))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    rpc_url_file: PathBuf,
    keeper_keypair_file: Option<PathBuf>,
    expected_program_elf_sha256_hex: String,
    maximum_epoch_record_rent_lamports: u64,
    maximum_transaction_fee_lamports: u64,
    minimum_keeper_balance_after_lamports: u64,
    compute_unit_limit: u32,
    micro_lamports_per_cu: u64,
    operation_timeout_seconds: u64,
    receipt_directory: PathBuf,
    acknowledge_healthy_collateral_forfeiture: bool,
}
fn load(path: &Path) -> Result<Config, Error> {
    let file = File::open(path).map_err(|_| Error::Configuration)?;
    let mut bytes = Vec::new();
    file.take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Configuration)?;
    if bytes.len() > 65_536 {
        return Err(Error::Configuration);
    }
    let c: Config = toml::from_str(std::str::from_utf8(&bytes).map_err(|_| Error::Configuration)?)
        .map_err(|_| Error::Configuration)?;
    if c.compute_unit_limit == 0
        || c.compute_unit_limit > 1_400_000
        || c.maximum_transaction_fee_lamports == 0
        || c.operation_timeout_seconds == 0
        || c.operation_timeout_seconds > 120
    {
        return Err(Error::Configuration);
    }
    hex_digest(&c.expected_program_elf_sha256_hex)?;
    Ok(c)
}
fn hex_digest(raw: &str) -> Result<[u8; 32], Error> {
    if raw.len() != 64 {
        return Err(Error::Configuration);
    }
    let mut value = [0; 32];
    for (i, pair) in raw.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let h = |b: u8| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(Error::Configuration),
        };
        value[i] = h(pair[0])? * 16 + h(pair[1])?;
    }
    if value == [0; 32] {
        return Err(Error::Configuration);
    }
    Ok(value)
}
#[cfg(target_os = "linux")]
fn private_directory(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::MetadataExt;
    if !path.is_absolute()
        || path.components().any(|p| {
            matches!(
                p,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(Error::PrivateFile);
    }
    let uid = fs::metadata("/proc/self")
        .map_err(|_| Error::PrivateFile)?
        .uid();
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component);
        let m = fs::symlink_metadata(&prefix).map_err(|_| Error::PrivateFile)?;
        if !m.is_dir()
            || m.file_type().is_symlink()
            || (m.uid() != 0 && m.uid() != uid)
            || (m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0)
        {
            return Err(Error::PrivateFile);
        }
    }
    let m = fs::symlink_metadata(path).map_err(|_| Error::PrivateFile)?;
    if m.uid() != uid || m.mode() & 0o7777 != 0o700 {
        return Err(Error::PrivateFile);
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn private_read(path: &Path) -> Result<Vec<u8>, Error> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    private_directory(path.parent().ok_or(Error::PrivateFile)?)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(0o400000 | 0o4000)
        .open(path)
        .map_err(|_| Error::PrivateFile)?;
    let m = file.metadata().map_err(|_| Error::PrivateFile)?;
    let current = fs::symlink_metadata(path).map_err(|_| Error::PrivateFile)?;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| Error::PrivateFile)?
        .uid();
    if !m.is_file()
        || m.uid() != uid
        || m.mode() & 0o7777 != 0o600
        || m.nlink() != 1
        || m.len() > 4096
        || current.file_type().is_symlink()
        || m.ino() != current.ino()
        || m.dev() != current.dev()
    {
        return Err(Error::PrivateFile);
    }
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::PrivateFile)?;
    if bytes.len() > 4096 {
        return Err(Error::PrivateFile);
    }
    Ok(bytes)
}
#[cfg(not(target_os = "linux"))]
fn private_read(_: &Path) -> Result<Vec<u8>, Error> {
    Err(Error::PrivateFile)
}
fn owned(account: Option<&AccountData>, size: usize) -> Result<&AccountData, Error> {
    let account = account.ok_or(Error::Account)?;
    if account.owner.to_bytes() != shortwin::ID.to_bytes()
        || account.executable
        || account.data.len() != size
    {
        return Err(Error::Account);
    }
    Ok(account)
}
fn plain<T: bytemuck::Pod + Discriminator>(account: Option<&AccountData>) -> Result<T, Error> {
    let a = owned(account, 8 + std::mem::size_of::<T>())?;
    if !a.data.starts_with(T::DISCRIMINATOR) {
        return Err(Error::Account);
    }
    Ok(bytemuck::pod_read_unaligned(&a.data[8..]))
}
struct RecoveryPlan {
    position: Pubkey,
    keeper: Pubkey,
    borrower: Pubkey,
    market: Pubkey,
    finance: Pubkey,
    exposure: Pubkey,
    vault: Pubkey,
    epoch_record: Pubkey,
    now: i64,
    slot: u64,
    default_at: i64,
    q_lost: u64,
    collateral: u64,
    keeper_payment: u64,
    lender_amount: u64,
    rent_due: u64,
    position_account_refund: u64,
    exposure_account_refund: u64,
    epoch_record_rent: u64,
    prefunded_epoch_swept: u64,
    max_epoch_rent: u64,
    program_deploy_slot: u64,
    program_elf_hash: String,
}
impl RecoveryPlan {
    fn json(&self) -> Value {
        json!({"schema":"shortwin-overdue-recovery-plan/v1","position":self.position.to_string(),"keeper":self.keeper.to_string(),
        "borrower":self.borrower.to_string(),"market":self.market.to_string(),"observed_slot":self.slot.to_string(),"actual_chain_timestamp":self.now,
        "default_at":self.default_at,"q_lost_raw":self.q_lost.to_string(),"collateral_lamports":self.collateral.to_string(),
        "keeper_payment_lamports":self.keeper_payment.to_string(),"lender_amount_lamports":self.lender_amount.to_string(),"borrower_collateral_return_lamports":"0",
        "rent_due_lamports":self.rent_due.to_string(),"rent_paid_claimed":false,"fair_value_claimed":false,"token_buyback":false,"price_feed_required":false,
        "position_account_refund_lamports":self.position_account_refund.to_string(),"exposure_account_refund_lamports":self.exposure_account_refund.to_string(),
        "epoch_record":self.epoch_record.to_string(),"epoch_record_rent_lamports":self.epoch_record_rent.to_string(),"maximum_epoch_record_rent_signed":self.max_epoch_rent.to_string(),
        "prefunded_epoch_swept_to_old_lenders_lamports":self.prefunded_epoch_swept.to_string(),"program_deploy_slot":self.program_deploy_slot.to_string(),"program_trimmed_elf_sha256":self.program_elf_hash,
        "transaction_signed":false,"transaction_submitted":false})
    }
    fn instruction(&self) -> Instruction {
        let (config, _) = Pubkey::find_program_address(
            &[GlobalConfig::SEED],
            &Pubkey::new_from_array(shortwin::ID.to_bytes()),
        );
        let (event_authority, _) = Pubkey::find_program_address(
            &[b"__event_authority"],
            &Pubkey::new_from_array(shortwin::ID.to_bytes()),
        );
        let accounts = shortwin::accounts::OverdueQuoteDefault {
            keeper: self.keeper,
            config,
            market: self.market,
            finance_market: self.finance,
            position: self.position,
            borrower: self.borrower,
            exposure: self.exposure,
            market_vault: self.vault,
            share_epoch_rec: self.epoch_record,
            system_program: Pubkey::default(),
            event_authority,
            program: shortwin::ID,
        };
        Instruction {
            program_id: shortwin::ID,
            accounts: accounts.to_account_metas(None),
            data: shortwin::instruction::OverdueQuoteDefault {
                max_epoch_record_rent: self.max_epoch_rent,
            }
            .data(),
        }
    }
}
async fn program_identity(chain: &RpcChain, expected: [u8; 32]) -> Result<(u64, String), Error> {
    let key = Pubkey::new_from_array(shortwin::ID.to_bytes());
    let accounts = chain.accounts(&[key]).await.map_err(|_| Error::Chain)?;
    let program = accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or(Error::ProgramIdentity)?;
    if !program.executable
        || program.owner.to_bytes() != program_data::LOADER_UPGRADEABLE
        || program.data.len() != 36
        || program.data[..4] != 2u32.to_le_bytes()
    {
        return Err(Error::ProgramIdentity);
    }
    let data_key = Pubkey::new_from_array(
        program.data[4..36]
            .try_into()
            .map_err(|_| Error::ProgramIdentity)?,
    );
    let (expected_data, _) = Pubkey::find_program_address(
        &[key.as_ref()],
        &Pubkey::new_from_array(program_data::LOADER_UPGRADEABLE),
    );
    if data_key != expected_data {
        return Err(Error::ProgramIdentity);
    }
    let accounts = chain
        .accounts(&[data_key])
        .await
        .map_err(|_| Error::Chain)?;
    let data = accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or(Error::ProgramIdentity)?;
    if data.executable
        || data.data.len() < program_data::HEADER_LEN
        || data.data.len() > 10 * 1024 * 1024
    {
        return Err(Error::ProgramIdentity);
    }
    let header = program_data::read(&data.owner.to_bytes(), &data.data)
        .map_err(|_| Error::ProgramIdentity)?;
    let elf = program_data::trimmed_elf(&data.owner.to_bytes(), &data.data)
        .map_err(|_| Error::ProgramIdentity)?;
    let hash: [u8; 32] = Sha256::digest(elf).into();
    if !elf.starts_with(b"\x7fELF") || hash != expected {
        return Err(Error::ProgramIdentity);
    }
    Ok((
        header.slot,
        hash.iter().map(|b| format!("{b:02x}")).collect(),
    ))
}
async fn plan(chain: &RpcChain, args: &Args, c: &Config) -> Result<RecoveryPlan, Error> {
    let genesis = chain
        .client()
        .get_genesis_hash()
        .await
        .map_err(|_| Error::Chain)?;
    if genesis.to_bytes() != MAINNET_GENESIS {
        return Err(Error::Chain);
    }
    let (program_deploy_slot, program_elf_hash) =
        program_identity(chain, hex_digest(&c.expected_program_elf_sha256_hex)?).await?;
    let first = chain
        .accounts(&[args.position])
        .await
        .map_err(|_| Error::Chain)?;
    let pos_account = owned(
        first.first().and_then(Option::as_ref),
        QuoteFinancePosition::SPACE,
    )?;
    let pos = QuoteFinancePosition::try_deserialize(&mut pos_account.data.as_slice())
        .map_err(|_| Error::Account)?;
    if !pos.is_pda(&args.position) {
        return Err(Error::Account);
    }
    let (config_key, _) = Pubkey::find_program_address(&[GlobalConfig::SEED], &shortwin::ID);
    let (finance_key, finance_bump) = Pubkey::find_program_address(
        &[QuoteFinanceMarket::SEED, pos.market.as_ref()],
        &shortwin::ID,
    );
    let (exposure_key, exposure_bump) = Pubkey::find_program_address(
        &[
            QuoteFinanceExposure::SEED,
            pos.market.as_ref(),
            pos.borrower.as_ref(),
        ],
        &shortwin::ID,
    );
    let (vault_key, vault_bump) =
        Pubkey::find_program_address(&[MarketVault::SEED, pos.market.as_ref()], &shortwin::ID);
    let linked = chain
        .accounts(&[config_key, pos.market, finance_key, exposure_key, vault_key])
        .await
        .map_err(|_| Error::Chain)?;
    let config: GlobalConfig = plain(linked.first().and_then(Option::as_ref))?;
    let mut market: Market = plain(linked.get(1).and_then(Option::as_ref))?;
    let finance_account = owned(
        linked.get(2).and_then(Option::as_ref),
        QuoteFinanceMarket::SPACE,
    )?;
    let mut finance = QuoteFinanceMarket::try_deserialize(&mut finance_account.data.as_slice())
        .map_err(|_| Error::Account)?;
    let exposure_account = owned(
        linked.get(3).and_then(Option::as_ref),
        QuoteFinanceExposure::SPACE,
    )?;
    let mut exposure = QuoteFinanceExposure::try_deserialize(&mut exposure_account.data.as_slice())
        .map_err(|_| Error::Account)?;
    let vault_account = owned(linked.get(4).and_then(Option::as_ref), MarketVault::SPACE)?;
    let vault: MarketVault = plain(Some(vault_account))?;
    if config.version != CONFIG_VERSION
        || market.version != MARKET_VERSION
        || !market.is_pda(&pos.market)
        || market.market_vault != vault_key
        || finance_key != pos.finance_market
        || finance.bump != finance_bump
        || vault.bump != vault_bump
        || exposure.bump != exposure_bump
        || exposure.market != pos.market
        || exposure.wallet != pos.borrower
    {
        return Err(Error::Account);
    }
    let (epoch_record, _) = Pubkey::find_program_address(
        &[
            ShareEpoch::SEED,
            pos.market.as_ref(),
            &market.share_epoch.to_le_bytes(),
        ],
        &shortwin::ID,
    );
    let clock = chain.clock().await.map_err(|_| Error::Chain)?;
    if clock.unix_timestamp < pos.default_at {
        return Err(Error::DefaultNotReady);
    }
    let mut pool = market.pool();
    let mut vault = vault.vault();
    quote_finance::accrue(
        &config_key,
        &pos.market,
        &mut market,
        &mut finance,
        &mut pool,
        &mut vault,
        clock.unix_timestamp,
    )
    .map_err(|_| Error::Booking)?;
    let booked = quote_finance::book_overdue_default(
        &config_key,
        &pos.market,
        &finance_key,
        &mut market,
        &mut finance,
        &mut pool,
        &mut vault,
        clock.unix_timestamp,
        &pos,
    )
    .map_err(|_| Error::Booking)?;
    quote_finance::remove_exposure(&mut exposure, pos.q, pos.n_open).map_err(|_| Error::Booking)?;
    let position_rent = chain
        .client()
        .get_minimum_balance_for_rent_exemption(QuoteFinancePosition::SPACE)
        .await
        .map_err(|_| Error::Chain)?;
    if pos_account.lamports < position_rent.checked_add(pos.v).ok_or(Error::Funding)? {
        return Err(Error::Funding);
    }
    let mut epoch_record_rent = 0;
    let mut prefunded_epoch_swept = 0;
    if booked.record.is_some() {
        epoch_record_rent = chain
            .client()
            .get_minimum_balance_for_rent_exemption(ShareEpoch::SPACE)
            .await
            .map_err(|_| Error::Chain)?;
        if epoch_record_rent > c.maximum_epoch_record_rent_lamports {
            return Err(Error::RentCap);
        }
        let epoch = chain
            .accounts(&[epoch_record])
            .await
            .map_err(|_| Error::Chain)?;
        if let Some(account) = epoch.first().and_then(Option::as_ref) {
            if account.owner != Pubkey::default() || !account.data.is_empty() || account.executable
            {
                return Err(Error::Account);
            }
            prefunded_epoch_swept = account.lamports;
        }
    }
    Ok(RecoveryPlan {
        position: args.position,
        keeper: args.keeper,
        borrower: pos.borrower,
        market: pos.market,
        finance: finance_key,
        exposure: exposure_key,
        vault: vault_key,
        epoch_record,
        now: clock.unix_timestamp,
        slot: clock.slot,
        default_at: pos.default_at,
        q_lost: pos.q,
        collateral: pos.v,
        keeper_payment: booked.keeper_payment,
        lender_amount: booked.lender_amount,
        rent_due: booked.rent_due,
        position_account_refund: pos_account.lamports - pos.v,
        exposure_account_refund: if exposure.q_open == 0 {
            exposure_account.lamports
        } else {
            0
        },
        epoch_record_rent,
        prefunded_epoch_swept,
        max_epoch_rent: if epoch_record_rent > 0 {
            c.maximum_epoch_record_rent_lamports
        } else {
            0
        },
        program_deploy_slot,
        program_elf_hash,
    })
}
fn unsigned(
    plan: &RecoveryPlan,
    c: &Config,
    hash: solana_hash::Hash,
) -> Result<VersionedTransaction, Error> {
    let mut ixs = vec![ComputeBudgetInstruction::set_compute_unit_limit(
        c.compute_unit_limit,
    )];
    if c.micro_lamports_per_cu > 0 {
        ixs.push(ComputeBudgetInstruction::set_compute_unit_price(
            c.micro_lamports_per_cu,
        ));
    }
    ixs.push(plan.instruction());
    let message = v0::Message::try_compile(&plan.keeper, &ixs, &[], hash)
        .map_err(|_| Error::Configuration)?;
    let tx = VersionedTransaction {
        signatures: vec![Signature::default(); usize::from(message.header.num_required_signatures)],
        message: VersionedMessage::V0(message),
    };
    if tx::size(&tx) > 1232 || tx.signatures.len() != 1 {
        return Err(Error::Configuration);
    }
    Ok(tx)
}
#[cfg(target_os = "linux")]
fn receipt(path: &Path, name: &str, value: &Value) -> Result<(), Error> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    private_directory(path).map_err(|_| Error::Receipt)?;
    let m = fs::symlink_metadata(path).map_err(|_| Error::Receipt)?;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| Error::Receipt)?
        .uid();
    if !path.is_absolute()
        || !m.is_dir()
        || m.file_type().is_symlink()
        || m.uid() != uid
        || m.mode() & 0o7777 != 0o700
    {
        return Err(Error::Receipt);
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| Error::Receipt)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(0o400000 | 0o4000)
        .open(path.join(name))
        .map_err(|_| Error::Receipt)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| Error::Receipt)?;
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| Error::Receipt)
}
#[cfg(not(target_os = "linux"))]
fn receipt(_: &Path, _: &str, _: &Value) -> Result<(), Error> {
    Err(Error::Receipt)
}
async fn run(args: Args, c: Config) -> Result<(), Error> {
    let rpc = String::from_utf8(private_read(&c.rpc_url_file)?).map_err(|_| Error::PrivateFile)?;
    let rpc = rpc.trim_end_matches(['\r', '\n']);
    let url = reqwest::Url::parse(rpc).map_err(|_| Error::Configuration)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Configuration);
    }
    let chain = RpcChain::new(rpc.to_owned());
    let reviewed = plan(&chain, &args, &c).await?;
    let (hash, last_valid_height) = chain
        .client()
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await
        .map_err(|_| Error::Chain)?;
    let mut transaction = unsigned(&reviewed, &c, hash)?;
    let fee = match &transaction.message {
        VersionedMessage::V0(message) => chain
            .client()
            .get_fee_for_message(message)
            .await
            .map_err(|_| Error::Chain)?,
        _ => return Err(Error::Configuration),
    };
    if fee > c.maximum_transaction_fee_lamports {
        return Err(Error::FeeCap);
    }
    let balance = chain
        .client()
        .get_balance(&args.keeper)
        .await
        .map_err(|_| Error::Chain)?;
    let maximum_debit = c
        .maximum_transaction_fee_lamports
        .checked_add(reviewed.max_epoch_rent)
        .and_then(|n| n.checked_add(c.minimum_keeper_balance_after_lamports))
        .ok_or(Error::Funding)?;
    if balance < maximum_debit {
        return Err(Error::Funding);
    }
    let simulation = chain
        .simulate(&transaction)
        .await
        .map_err(|_| Error::Simulation)?;
    if !simulation.ok() || simulation.units_consumed > u64::from(c.compute_unit_limit) {
        return Err(Error::Simulation);
    }
    let mut review = reviewed.json();
    review["network_fee_lamports"] = json!(fee.to_string());
    review["maximum_network_fee_lamports"] = json!(c.maximum_transaction_fee_lamports.to_string());
    review["simulation_units"] = json!(simulation.units_consumed);
    review["last_valid_block_height"] = json!(last_valid_height.to_string());
    if !args.send {
        println!("{review}");
        return Ok(());
    }
    if !c.acknowledge_healthy_collateral_forfeiture {
        return Err(Error::Configuration);
    }
    let key = private_read(c.keeper_keypair_file.as_deref().ok_or(Error::Keypair)?)?;
    let raw: Vec<u8> = serde_json::from_slice(&key).map_err(|_| Error::Keypair)?;
    let payer = Keypair::try_from(raw.as_slice()).map_err(|_| Error::Keypair)?;
    if payer.pubkey() != args.keeper {
        return Err(Error::Keypair);
    }
    transaction.signatures[0] = payer.sign_message(&transaction.message.serialize());
    let signature = transaction.signatures[0];
    review["transaction_signed"] = json!(true);
    review["signature"] = json!(signature.to_string());
    review["signed_message_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(transaction.message.serialize())
    ));
    let stem = format!("{}-{}", args.position, signature);
    receipt(
        &c.receipt_directory,
        &format!("{stem}.prepared.json"),
        &review,
    )?;
    println!("{review}"); // public signature remains visible even if the send deadline cancels

    // Exactly one send. A lost/uncertain confirmation retains prepared signature;
    // inspect that exact public signature before explicitly invoking a new command.
    let result = chain.send(&transaction).await;
    let (outcome, disposition) = match result {
        Ok(outcome)
            if outcome.signature == signature
                && outcome.slot > 0
                && !matches!(outcome.err, Some(TxError::Dropped(_))) =>
        {
            let disposition = if outcome.ok() {
                "confirmed_success"
            } else {
                "confirmed_failure"
            };
            (Some(outcome), disposition)
        }
        _ => (None, "confirmation_uncertain"),
    };
    let result = json!({"schema":"shortwin-overdue-recovery-result/v1","signature":signature.to_string(),"position":args.position.to_string(),
        "disposition":disposition,"slot":outcome.as_ref().map(|o|o.slot.to_string()),"actual_fee_lamports":outcome.as_ref().and_then(|o|(o.fee>0).then(||o.fee.to_string())),
        "transaction_submitted":true,"new_signature_retry":false});
    receipt(
        &c.receipt_directory,
        &format!("{stem}.result.json"),
        &result,
    )?;
    println!("{result}");
    match disposition {
        "confirmed_success" => Ok(()),
        "confirmed_failure" => Err(Error::ConfirmedFailure),
        _ => Err(Error::ConfirmationUncertain),
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = match args(std::env::args_os().skip(1)) {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!(
                "{USAGE}\nDefault is read-only planning/simulation; price authority keys are not used."
            );
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(1);
        }
    };
    let c = match load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    match tokio::time::timeout(
        Duration::from_secs(c.operation_timeout_seconds),
        run(args, c),
    )
    .await
    {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
        Err(_) => {
            eprintln!(
                "{}; inspect any prepared signature before retry",
                Error::ConfirmationUncertain
            );
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            rpc_url_file: "not-read-by-pure-tests".into(),
            keeper_keypair_file: Some("missing-key-not-loaded-in-plan".into()),
            expected_program_elf_sha256_hex: "11".repeat(32),
            maximum_epoch_record_rent_lamports: 5_000_000,
            maximum_transaction_fee_lamports: 20_000,
            minimum_keeper_balance_after_lamports: 10_000_000,
            compute_unit_limit: 200_000,
            micro_lamports_per_cu: 10_000,
            operation_timeout_seconds: 30,
            receipt_directory: "not-written".into(),
            acknowledge_healthy_collateral_forfeiture: true,
        }
    }
    fn plan(keeper: Pubkey) -> RecoveryPlan {
        RecoveryPlan {
            position: Pubkey::new_unique(),
            keeper,
            borrower: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            finance: Pubkey::new_unique(),
            exposure: Pubkey::new_unique(),
            vault: Pubkey::new_unique(),
            epoch_record: Pubkey::new_unique(),
            now: 200_000,
            slot: 800_000,
            default_at: 186_400,
            q_lost: u64::MAX,
            collateral: 10_000,
            keeper_payment: 100,
            lender_amount: 9_900,
            rent_due: 1_000,
            position_account_refund: 2_000_000,
            exposure_account_refund: 1_000_000,
            epoch_record_rent: 3_000_000,
            prefunded_epoch_swept: 777,
            max_epoch_rent: 5_000_000,
            program_deploy_slot: 123,
            program_elf_hash: "11".repeat(32),
        }
    }
    #[test]
    fn default_mode_never_enables_send_or_accepts_foreign_transactions() {
        let pos = Pubkey::new_unique().to_string();
        let keeper = Pubkey::new_unique().to_string();
        let raw = [
            "--config",
            "settings.toml",
            "--position",
            pos.as_str(),
            "--keeper",
            keeper.as_str(),
        ];
        let parsed = args(raw.map(OsString::from)).unwrap().unwrap();
        assert!(!parsed.send);
        let mut active = raw.map(OsString::from).to_vec();
        active.push("--send".into());
        assert!(args(active).unwrap().unwrap().send);
        for bad in ["--transaction", "--keypair", "--quote", "--simulate-only"] {
            let mut bad_args = raw.map(OsString::from).to_vec();
            bad_args.push(bad.into());
            assert!(args(bad_args).is_err());
        }
    }
    #[test]
    fn exact_program_instruction_binds_all_destinations_and_signed_epoch_record_cap() {
        let keeper = Keypair::new();
        let plan = plan(keeper.pubkey());
        let ix = plan.instruction();
        assert_eq!(ix.program_id, shortwin::ID);
        assert_eq!(ix.accounts.len(), 12);
        assert_eq!(ix.accounts[0].pubkey, keeper.pubkey());
        assert!(ix.accounts[0].is_signer);
        assert!(ix.accounts[0].is_writable);
        assert_eq!(ix.accounts.iter().filter(|a| a.is_signer).count(), 1);
        assert_eq!(ix.accounts[4].pubkey, plan.position);
        assert_eq!(ix.accounts[5].pubkey, plan.borrower);
        assert_eq!(ix.accounts[7].pubkey, plan.vault);
        assert_eq!(ix.accounts[8].pubkey, plan.epoch_record);
        assert_eq!(
            &ix.data[..8],
            shortwin::instruction::OverdueQuoteDefault::DISCRIMINATOR
        );
        assert_eq!(&ix.data[8..], &5_000_000u64.to_le_bytes());
        let mut tx = unsigned(&plan, &config(), solana_hash::Hash::new_unique()).unwrap();
        assert_eq!(tx.signatures, vec![Signature::default()]);
        assert!(tx::size(&tx) <= 1_232);
        tx.signatures[0] = keeper.sign_message(&tx.message.serialize());
        assert!(tx.signatures[0].verify(keeper.pubkey().as_ref(), &tx.message.serialize()));
        assert_eq!(tx.message.instructions().len(), 3);
        let plan = RecoveryPlan {
            max_epoch_rent: 0,
            ..plan
        };
        assert_eq!(&plan.instruction().data[8..], &0u64.to_le_bytes());
    }
    #[test]
    fn plan_labels_loss_compensation_refunds_and_unknown_trade_values_exactly() {
        let plan = plan(Pubkey::new_unique());
        let view = plan.json();
        assert_eq!(view["q_lost_raw"], "18446744073709551615");
        assert_eq!(view["lender_amount_lamports"], "9900");
        assert_eq!(view["borrower_collateral_return_lamports"], "0");
        assert_eq!(view["position_account_refund_lamports"], "2000000");
        assert_eq!(view["prefunded_epoch_swept_to_old_lenders_lamports"], "777");
        for key in [
            "transaction_signed",
            "transaction_submitted",
            "token_buyback",
            "price_feed_required",
            "fair_value_claimed",
            "rent_paid_claimed",
        ] {
            assert_eq!(view[key], false);
        }
        assert!(hex_digest("REPLACE_WITH_FINAL_REVIEWED_TRIMMED_ELF_HASH").is_err());
        assert!(!format!("{}", Error::Keypair).contains("missing-key"));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn receipts_and_secrets_are_private_and_existing_receipts_are_never_overwritten() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "shortwin-recovery-cli-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        receipt(
            &dir,
            "public.json",
            &json!({"signature":"public-test-signature"}),
        )
        .unwrap();
        let path = dir.join("public.json");
        let before = fs::read(&path).unwrap();
        assert!(receipt(&dir, "public.json", &json!({"replace":true})).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
        assert_eq!(private_read(&path).unwrap(), before);
        let alias = dir.join("alias");
        symlink(&path, &alias).unwrap();
        assert!(private_read(&alias).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(private_read(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
