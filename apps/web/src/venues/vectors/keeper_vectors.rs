//! Keeper-derived venue vectors for the web venue layer (FRONTEND.md §10.1 SHORT: "the venue account
//! lists against vectors produced by the keeper's `venue.rs` for each venue").
//!
//! This file is the generator of `keeper-vectors.json` next to it. It is not compiled by the web
//! build: to regenerate, copy it into the keeper crate as an example and run it there (TEST-ONLY
//! `test-venue` feature for the mock venue):
//!
//! ```sh
//! cp apps/web/src/venues/vectors/keeper_vectors.rs services/keeper/examples/web_vectors.rs
//! cd services && cargo run -p shortwin-keeper --features test-venue --example web_vectors \
//!     > ../apps/web/src/venues/vectors/keeper-vectors.json
//! rm keeper/examples/web_vectors.rs
//! ```
//!
//! Every list comes from the keeper's own functions (`shortwin_keeper::venue::{read_set,
//! read_metas, trade_extras, extras_candidates, pamm_creator_vault_prep}`), which build the
//! program's lists from the program's own key functions; the market caches are written by the
//! program's listing helpers (`pump_curve::write_caches`, `pump_amm::precompute`), so the inputs are
//! what a real market holds. Venue-state accounts are the dumped mainnet fixtures of
//! `tests/fixtures/accounts` (by path) where the keeper reads them.

use std::fmt::Write as _;

use anchor_lang::prelude::Pubkey;
use shortwin::state::{
    ENTRY_FIXED, ENTRY_TEMPLATE, EXTRAS_IX_BUY, EXTRAS_IX_SELL, ExtraEntry, ExtrasSet, IxExtras,
    MARKET_FLAG_SOL_IS_TOKEN0_OR_COIN, Market, PLACEHOLDER_MINT, PLACEHOLDER_POOL,
    PLACEHOLDER_TRADER, SEED_CONST, SEED_NONE, SEED_PLACEHOLDER, SeedPart, VenueExtras, VenueKind,
    raydium_cache,
};
use shortwin::venues::{pump_amm, pump_curve};
use shortwin_keeper::venue;
use shortwin_services_common::AccountData;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/");

fn pk(s: &str) -> Pubkey {
    s.parse().expect("base58")
}

/// A deterministic key from a label (no RNG: the vectors are stable): four FNV-1a 64 lanes.
fn key(label: &str) -> Pubkey {
    let mut out = [0u8; 32];
    for (lane, chunk) in out.chunks_mut(8).enumerate() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ (lane as u64);
        for b in label.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        chunk.copy_from_slice(&h.to_le_bytes());
    }
    Pubkey::new_from_array(out)
}

fn fixture(path: &str, owner: &str) -> AccountData {
    let data = std::fs::read(format!("{FIXTURES}{path}")).expect("fixture");
    AccountData {
        lamports: 1,
        owner: pk(owner),
        executable: false,
        data,
    }
}

const PUMP: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PAMM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// The market's own PDAs as `create_market` derives them.
fn base_market(mint: Pubkey, token_program: Pubkey) -> Market {
    let mut m: Market = bytemuck::Zeroable::zeroed();
    let market = Pubkey::find_program_address(&[Market::SEED, mint.as_ref()], &shortwin::ID).0;
    m.mint = mint;
    m.token_program = token_program;
    m.trader =
        Pubkey::find_program_address(&[shortwin::state::TRADER_SEED, market.as_ref()], &shortwin::ID).0;
    m.trader_wsol = Pubkey::find_program_address(
        &[shortwin::state::TRADER_WSOL_SEED, market.as_ref()],
        &shortwin::ID,
    )
    .0;
    m.trader_base_ata =
        anchor_spl_ata(&m.trader, &mint, &token_program);
    m
}

fn anchor_spl_ata(owner: &Pubkey, mint: &Pubkey, program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), program.as_ref(), mint.as_ref()],
        &shortwin::constants::ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

// ---- JSON (by hand: no serde_json dependency needed in the example) ------------------------

fn s(k: &Pubkey) -> String {
    format!("\"{k}\"")
}

fn keys(v: &[Pubkey]) -> String {
    format!("[{}]", v.iter().map(s).collect::<Vec<_>>().join(","))
}

fn metas(v: &[anchor_lang::solana_program::instruction::AccountMeta]) -> String {
    format!(
        "[{}]",
        v.iter()
            .map(|m| format!("{{\"key\":{},\"w\":{}}}", s(&m.pubkey), m.is_writable))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn market_json(m: &Market) -> String {
    let cache: Vec<Pubkey> = m.pool_cache.to_vec();
    format!(
        "{{\"mint\":{},\"tokenProgram\":{},\"trader\":{},\"traderWsol\":{},\"traderBaseAta\":{},\
         \"bondingCurve\":{},\"canonicalPool\":{},\"curveBaseAta\":{},\"curveQuoteAta\":{},\
         \"sharingConfig\":{},\"pumpUva\":{},\"pumpUvaQuoteAta\":{},\"cachedCreator\":{},\
         \"cachedCreatorVault\":{},\"cachedCreatorVaultAta\":{},\"traderWsolAta\":{},\
         \"pumpPoolAuthority\":{},\"flags\":{},\"poolCache\":{}}}",
        s(&m.mint),
        s(&m.token_program),
        s(&m.trader),
        s(&m.trader_wsol),
        s(&m.trader_base_ata),
        s(&m.bonding_curve),
        s(&m.canonical_pool),
        s(&m.curve_base_ata),
        s(&m.curve_quote_ata),
        s(&m.sharing_config),
        s(&m.pump_uva),
        s(&m.pump_uva_quote_ata),
        s(&m.cached_creator),
        s(&m.cached_creator_vault),
        s(&m.cached_creator_vault_ata),
        s(&m.trader_wsol_ata),
        s(&m.pump_pool_authority),
        m.flags,
        keys(&cache),
    )
}

/// One vector: the market, the ReadSet accounts the keeper read (fixture paths), the lists.
fn case(
    out: &mut String,
    name: &str,
    kind: VenueKind,
    m: &Market,
    read_files: &[(usize, &str, &str)],
) {
    let set = venue::read_set(kind, m).expect("read set");
    let trading = venue::read_metas(kind, &set, true).expect("metas");
    let crank = venue::read_metas(kind, &set, false).expect("metas");
    let mut read: Vec<Option<AccountData>> = vec![None; set.len()];
    for (i, path, owner) in read_files {
        read[*i] = Some(fixture(path, owner));
    }
    let trade = match kind {
        VenueKind::None | VenueKind::PumpMigrating => None,
        _ => Some(venue::trade_extras(kind, m, &read).expect("trade extras")),
    };
    let payer = key("payer");
    let prep = venue::pamm_creator_vault_prep(&payer, m, kind, &read);
    let files = read_files
        .iter()
        .map(|(i, p, o)| format!("{{\"slot\":{i},\"file\":\"{p}\",\"owner\":\"{o}\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    let _ = write!(
        out,
        "{{\"name\":\"{name}\",\"kind\":{},\"market\":{},\"read\":[{files}],\"readSet\":{},\
         \"tradeMetas\":{},\"crankMetas\":{},\"tradeExtras\":{},\"creatorVaultPrep\":{}}}",
        kind as u8,
        market_json(m),
        keys(&set),
        metas(&trading),
        metas(&crank),
        trade.as_deref().map_or("null".to_string(), metas),
        prep.map_or("null".to_string(), |ix| format!(
            "{{\"payer\":{},\"keys\":{}}}",
            s(&payer),
            keys(&ix.accounts.iter().map(|a| a.pubkey).collect::<Vec<_>>())
        )),
    );
}

fn seed(kind: u8, len: u8, bytes: &[u8]) -> SeedPart {
    let mut b = [0u8; 32];
    b[..bytes.len()].copy_from_slice(bytes);
    SeedPart {
        kind,
        len,
        bytes: b,
    }
}

fn none_seed() -> SeedPart {
    seed(SEED_NONE, 0, &[])
}

fn entry_fixed(k: Pubkey, writable: bool) -> ExtraEntry {
    ExtraEntry {
        kind: ENTRY_FIXED,
        writable: u8::from(writable),
        n_seeds: 0,
        _pad: [0; 5],
        key: k,
        seeds: [none_seed(), none_seed(), none_seed(), none_seed()],
    }
}

fn entry_template(program: Pubkey, parts: [SeedPart; 4], n: u8, writable: bool) -> ExtraEntry {
    ExtraEntry {
        kind: ENTRY_TEMPLATE,
        writable: u8::from(writable),
        n_seeds: n,
        _pad: [0; 5],
        key: program,
        seeds: parts,
    }
}

fn set_of(entries: &[ExtraEntry]) -> ExtrasSet {
    let mut s: ExtrasSet = bytemuck::Zeroable::zeroed();
    s.version = 1;
    s.count = u8::try_from(entries.len()).expect("≤ 4");
    for (i, e) in entries.iter().enumerate() {
        s.entries[i] = *e;
    }
    s
}

fn entry_json(e: &ExtraEntry) -> String {
    let seeds = e
        .seeds
        .iter()
        .map(|p| {
            format!(
                "{{\"kind\":{},\"len\":{},\"bytes\":[{}]}}",
                p.kind,
                p.len,
                p.bytes.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"kind\":{},\"writable\":{},\"nSeeds\":{},\"key\":{},\"seeds\":[{seeds}]}}",
        e.kind,
        e.writable,
        e.n_seeds,
        s(&e.key)
    )
}

fn set_json(set: &ExtrasSet) -> String {
    format!(
        "{{\"count\":{},\"entries\":[{}]}}",
        set.count,
        set.entries.iter().map(entry_json).collect::<Vec<_>>().join(",")
    )
}

fn extras_case(out: &mut String, name: &str, kind: VenueKind, m: &Market, ve: &VenueExtras) {
    let lists = [EXTRAS_IX_SELL, EXTRAS_IX_BUY]
        .iter()
        .map(|ix| {
            let c = venue::extras_candidates(kind, *ix, m, Some(ve));
            format!(
                "[{}]",
                c.iter()
                    .map(|ch| format!(
                        "{{\"set\":{},\"bumps\":[{}],\"metas\":{}}}",
                        ch.sel.set,
                        ch.sel.bumps.iter().map(u8::to_string).collect::<Vec<_>>().join(","),
                        metas(&ch.metas)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>();
    let sets = ve
        .sets
        .iter()
        .map(|ixe| {
            format!(
                "{{\"current\":{},\"previous\":{},\"stable\":{}}}",
                set_json(&ixe.current),
                set_json(&ixe.previous),
                set_json(&ixe.stable)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let _ = write!(
        out,
        "{{\"name\":\"{name}\",\"kind\":{},\"market\":{},\"venueExtras\":{},\"sets\":[{sets}],\
         \"sell\":{},\"buy\":{}}}",
        kind as u8,
        market_json(m),
        s(&shortwin_keeper::pda::venue_extras(kind as u8)),
        lists[0],
        lists[1]
    );
}

fn main() {
    let mut cases: Vec<String> = Vec::new();

    // Mock (TEST-ONLY): the pool cache's pool / token vault / SOL vault.
    let mut mock = base_market(key("mock-mint"), pk(TOKEN));
    mock.pool_cache[raydium_cache::RAY_POOL] = key("mock-pool");
    mock.pool_cache[raydium_cache::RAY_VAULT_TOK] = key("mock-vault-tok");
    mock.pool_cache[raydium_cache::RAY_VAULT_SOL] = key("mock-vault-sol");
    let mut c = String::new();
    case(&mut c, "mock", VenueKind::Mock, &mock, &[]);
    cases.push(c);

    // Raydium AMM v4 and CPMM, both SOL sides, both token programs (CPMM).
    for (sol_first, program) in [(false, TOKEN), (true, TOKEN), (false, TOKEN_2022), (true, TOKEN_2022)] {
        let mut m = base_market(key(&format!("ray-mint-{sol_first}-{program}")), pk(program));
        for (i, l) in [
            (raydium_cache::RAY_POOL, "pool"),
            (raydium_cache::RAY_VAULT_SOL, "vault-sol"),
            (raydium_cache::RAY_VAULT_TOK, "vault-tok"),
            (raydium_cache::RAY_AMM_CONFIG, "config"),
            (raydium_cache::RAY_OBSERVATION, "observation"),
            (raydium_cache::RAY_LP_MINT, "lp"),
            (raydium_cache::RAY_TOK_PROGRAM, "tok-program"),
        ] {
            m.pool_cache[i] = key(&format!("ray-{l}-{sol_first}-{program}"));
        }
        if sol_first {
            m.flags |= MARKET_FLAG_SOL_IS_TOKEN0_OR_COIN;
        }
        if program == TOKEN {
            let mut c = String::new();
            case(&mut c, &format!("raydium_v4-solfirst-{sol_first}"), VenueKind::RaydiumV4, &m, &[]);
            cases.push(c);
        }
        let mut c = String::new();
        case(
            &mut c,
            &format!("raydium_cpmm-solfirst-{sol_first}-{}", if program == TOKEN { "spl" } else { "t22" }),
            VenueKind::RaydiumCpmm,
            &m,
            &[],
        );
        cases.push(c);
    }

    // Pump curve: the `fresh` fixture coin, listed (the program's own cache writers), the live
    // `Global` and the curve as dumped — pump v3's 10-key list (pump-final §6.1.1); then the same
    // market whose cached creator differs from the live curve's (v3 names no creator vault: the
    // same list).
    let fresh_mint = pk("4mNDC2YAJRWsnbC1mS4KkiLbUorVwLJ7PBsJX7Vzpump");
    let mut curve_m = base_market(fresh_mint, pk(TOKEN_2022));
    curve_m.bonding_curve = pk("6mXpsJNUoaGpPmdHKhtwcAoUqDS6KzsHL6dwTryCMG8v");
    curve_m.canonical_pool =
        shortwin::instructions::create_market::canonical_pool_key(&fresh_mint);
    pump_curve::write_caches(&mut curve_m, &pk("96mv1v9yhyrZxpt6SRjQ5BUgsfqRtSCTv9QJ3L1ui4qh"))
        .expect("caches");
    let curve_read = [
        (pump_curve::C_CURVE, "accounts/pump/pump_fresh_curve.bin", PUMP),
        (pump_curve::C_GLOBAL, "accounts/pump/pump_global.bin", PUMP),
    ];
    for kind in [VenueKind::PumpCurve, VenueKind::PumpMigrating] {
        let mut c = String::new();
        let name = if kind == VenueKind::PumpCurve { "pump_curve" } else { "pump_migrating" };
        case(&mut c, name, kind, &curve_m, &curve_read);
        cases.push(c);
    }
    let mut other = curve_m;
    pump_curve::write_cache(&mut other, &key("another-creator"));
    let mut c = String::new();
    case(&mut c, "pump_curve-creator-changed", VenueKind::PumpCurve, &other, &curve_read);
    cases.push(c);

    // PumpSwap: the `fresh` and `cashback` canonical pools as dumped, the live GlobalConfig;
    // the cache precomputed for the pool's coin creator, then for another (the creator vault and
    // its WSOL ATA re-derived, and the exit's `create_idempotent(T11)` prep, M6-SEC-4).
    for (role, mint, pool, creator_from_pool) in [
        (
            "fresh",
            "6Rs3yJGQapP2FCgBv2Z6eiH99VJSGSGBvqmPCm13pump",
            "5768Z3VczMXvSXcC3Cco6fCbdf34RAy2f97w46knYi7o",
            true,
        ),
        (
            "cashback",
            "34Bkw9UMYRQNFC4BB7E2fPR9U7GTx6oFZZFgP5wYTMAh",
            "xA3sC2FBSGAQLtnetteMWzvbJxYNQr1j9199QidGFdo",
            false,
        ),
    ] {
        let file = format!("accounts/pamm/pamm_{role}_pool.bin");
        let pool_acc = fixture(&file, PAMM);
        let parsed = shortwin_venue_state::pump_amm::read_pool(&pk(PAMM).to_bytes(), &pool_acc.data)
            .expect("pool");
        let live_creator = Pubkey::new_from_array(parsed.coin_creator);
        let cached = if creator_from_pool { live_creator } else { key("stale-creator") };
        let mut m = base_market(pk(mint), pk(TOKEN_2022));
        m.canonical_pool = pk(pool);
        m.bonding_curve = Pubkey::find_program_address(
            &[b"bonding-curve", pk(mint).as_ref()],
            &pk(PUMP),
        )
        .0;
        pump_amm::precompute(&mut m, &cached).expect("precompute");
        let file: &'static str = Box::leak(file.into_boxed_str());
        let mut c = String::new();
        case(
            &mut c,
            &format!("pump_swap-{role}"),
            VenueKind::PumpSwap,
            &m,
            &[
                (pump_amm::P_POOL, file, PAMM),
                (pump_amm::P_GLOBAL_CONFIG, "accounts/pump/pamm_global_config.bin", PAMM),
            ],
        );
        cases.push(c);
    }

    // List-only: the detection set.
    let mut none = base_market(key("none-mint"), pk(TOKEN));
    none.bonding_curve = key("none-curve");
    none.canonical_pool = key("none-pool");
    let mut c = String::new();
    case(&mut c, "none", VenueKind::None, &none, &[]);
    cases.push(c);

    // Extras (A41): a current set with a fixed entry and a template on the mint, pool and trader
    // placeholders; a previous set with one fixed entry; no stable set — on the mock and on
    // Raydium v4 (the `pool` placeholder resolves to the pool cache's pool on both).
    let mut ve: VenueExtras = bytemuck::Zeroable::zeroed();
    ve.version = shortwin::constants::VENUE_EXTRAS_VERSION;
    let tmpl_program = key("extras-program");
    let current = set_of(&[
        entry_fixed(key("extras-fixed"), true),
        entry_template(
            tmpl_program,
            [
                seed(SEED_CONST, 5, b"extra"),
                seed(SEED_PLACEHOLDER, PLACEHOLDER_MINT, &[]),
                seed(SEED_PLACEHOLDER, PLACEHOLDER_POOL, &[]),
                seed(SEED_PLACEHOLDER, PLACEHOLDER_TRADER, &[]),
            ],
            4,
            false,
        ),
    ]);
    let previous = set_of(&[entry_fixed(key("extras-previous"), false)]);
    ve.sets[EXTRAS_IX_SELL] = IxExtras {
        current,
        previous,
        stable: bytemuck::Zeroable::zeroed(),
        current_since: 1,
    };
    ve.sets[EXTRAS_IX_BUY] = IxExtras {
        current: previous,
        previous: bytemuck::Zeroable::zeroed(),
        stable: current,
        current_since: 1,
    };
    for (name, kind, m) in [("mock", VenueKind::Mock, &mock), ("raydium_v4", VenueKind::RaydiumV4, &{
        let mut m = base_market(key("ray-extras-mint"), pk(TOKEN));
        m.pool_cache[raydium_cache::RAY_POOL] = key("ray-extras-pool");
        m
    })] {
        ve.venue = kind as u8;
        ve.bump = Pubkey::find_program_address(&[VenueExtras::SEED, &[kind as u8]], &shortwin::ID).1;
        let mut c = String::new();
        extras_case(&mut c, &format!("extras-{name}"), kind, m, &ve);
        cases.push(c);
    }

    println!(
        "{{\"generator\":\"apps/web/src/venues/vectors/keeper_vectors.rs\",\"cases\":[{}]}}",
        cases.join(",\n")
    );
}
