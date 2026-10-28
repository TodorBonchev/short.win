//! Actual new-program exits through the pinned Router V2 ELF. All loans are first
//! opened by the real financial handler; historical state/test ALT are local only.
use crate::{
    assert_ok, deterministic_key, env as pda,
    jupiter::HistoricalRoute,
    position_case::{exposure, instruction as ix, loan, PositionCase as Case, Q},
    tokens::{self, TokenProgram},
};
use anchor_lang::prelude::Pubkey;
use shortwin_simple::{
    accounts,
    exits::{CloseArgs, ForcedExitArgs},
    instruction,
    reference::PublishReferenceArgs,
    route::{self, RouteArgs},
    state::PriceReference,
};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;

pub fn opened() -> Case {
    let mut c = Case::new();
    let i = c.ix(c.args());
    assert_ok(c.env.send_v0(&[i], &[&c.borrower]).0);
    c
}
pub fn key(c: &Case) -> Pubkey {
    loan(c.pool, c.borrower.pubkey(), 0)
}
pub fn position(c: &Case) -> accounts::PositionAccounts {
    accounts::PositionAccounts {
        config: pda::config(),
        policy: pda::policy(1),
        market: c.market,
        pool: c.pool,
        loan: key(c),
        exposure: exposure(c.pool, c.borrower.pubkey()),
        borrower: c.borrower.pubkey(),
    }
}
pub fn token(c: &Case) -> accounts::LoanTokenAccounts {
    accounts::LoanTokenAccounts {
        mint: c.route.base_mint,
        token_vault: pda::token_vault(&c.pool),
        token_program: route::TOKEN_ID,
    }
}
pub fn scratch(c: &Case, seed: &[u8]) -> Pubkey {
    Pubkey::find_program_address(&[seed, key(c).as_ref()], &shortwin_simple::ID).0
}
pub fn execution(c: &Case) -> accounts::RouteExecution {
    accounts::RouteExecution {
        execution_authority: scratch(c, route::AUTHORITY_SEED),
        execution_base: scratch(c, route::BASE_SEED),
        execution_wsol: scratch(c, route::WSOL_SEED),
        wsol_mint: route::WSOL_MINT,
        wsol_token_program: route::TOKEN_ID,
        jupiter: route::JUPITER_ID,
        jupiter_program_data: route::JUPITER_PROGRAM_DATA,
        system_program: anchor_lang::system_program::ID,
    }
}
pub fn route_accounts(c: &Case) -> Vec<solana_instruction::AccountMeta> {
    c.route.accounts(
        scratch(c, route::AUTHORITY_SEED),
        scratch(c, route::BASE_SEED),
        scratch(c, route::WSOL_SEED),
        true,
    )
}
pub fn close(c: &Case, input: u64, minimum: u64) -> Instruction {
    let a = accounts::ClosePosition {
        actor: c.borrower.pubkey(),
        position: position(c),
        token: token(c),
        execution: execution(c),
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    let args = CloseArgs {
        max_buyback_cost: input,
        temporary_funding: 100_000_000,
        route: RouteArgs {
            input_amount: input,
            data: HistoricalRoute::data(input, minimum),
        },
    };
    let mut i = ix(a, instruction::ClosePosition { args });
    i.accounts.extend(route_accounts(c));
    i
}
pub fn force(c: &Case, keeper: &Keypair, input: u64) -> Instruction {
    let a = accounts::ForceExit {
        actor: keeper.pubkey(),
        position: position(c),
        token: token(c),
        execution: execution(c),
        reference: c.reference,
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    let args = ForcedExitArgs {
        max_buyback_cost: input,
        temporary_funding: 100_000_000,
        minimum_reference_sequence: 1,
        route: RouteArgs {
            input_amount: input,
            data: HistoricalRoute::data(input, Q),
        },
    };
    let mut i = ix(a, instruction::ForceExit { args });
    i.accounts.extend(route_accounts(c));
    i
}
pub fn default_ix(c: &Case, keeper: &Keypair) -> Instruction {
    ix(
        accounts::DefaultLoan {
            actor: keeper.pubkey(),
            position: position(c),
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        },
        instruction::DefaultLoan {},
    )
}
pub fn add(c: &Case, amount: u64) -> Instruction {
    ix(
        accounts::AddMargin {
            actor: c.borrower.pubkey(),
            position: position(c),
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        },
        instruction::AddMargin { amount },
    )
}
pub fn arm(c: &Case, keeper: &Keypair) -> Instruction {
    ix(
        accounts::ArmLiquidation {
            actor: keeper.pubkey(),
            position: position(c),
            reference: c.reference,
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        },
        instruction::ArmLiquidation {
            minimum_reference_sequence: 1,
        },
    )
}
pub fn source(c: &mut Case, tag: u8) -> Pubkey {
    let k = deterministic_key(tag);
    let address = tokens::create_account(
        &mut c.env,
        &k,
        &c.route.base_mint,
        &c.borrower.pubkey(),
        TokenProgram::Spl,
    );
    let mut a = c.env.svm.get_account(&address).unwrap();
    a.data[64..72].copy_from_slice(&(2 * Q).to_le_bytes());
    c.env.svm.set_account(address, a).unwrap();
    address
}
pub fn repay(c: &Case, source: Pubkey) -> Instruction {
    ix(
        accounts::Repay {
            actor: c.borrower.pubkey(),
            position: position(c),
            token: token(c),
            source,
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        },
        instruction::Repay {},
    )
}
pub fn keeper(c: &mut Case) -> Keypair {
    let keeper = deterministic_key(110);
    c.env.fund(keeper.pubkey(), 1_000_000_000);
    keeper
}
pub fn clock(c: &mut Case, ts: i64, slot: u64) {
    c.env.set_clock(ts, slot);
}
pub fn publish(c: &mut Case, ts: i64, slot: u64, bid: u64, ask: u64) {
    let r: PriceReference = c.env.read(&c.reference);
    let epoch = ts / 60 * 60;
    let args = PublishReferenceArgs {
        quantity: Q,
        sequence: r.sequence + 1,
        source_slot: slot,
        source_ts: ts,
        finite: true,
        live_bid_lower: bid,
        live_ask_upper: ask,
        history_ask_upper_max: ask,
        history_bid_lower_max: bid,
        acquisition_reference: ask,
        completed_epochs: 2,
        history_start_ts: epoch - 120,
        history_end_ts: epoch - 1,
        evidence_hash: [9; 32],
        max_reference_rent: 100_000_000,
    };
    let reporter = c.env.reporter.insecure_clone();
    let a = accounts::PublishReference {
        reporter: reporter.pubkey(),
        config: pda::config(),
        market: c.market,
        policy: pda::policy(1),
        reference: c.reference,
        system_program: anchor_lang::system_program::ID,
        event_authority: pda::event_authority(),
        program: shortwin_simple::ID,
    };
    assert_ok(c.env.send(
        &[ix(a, instruction::PublishReference { args })],
        &[&reporter],
    ));
}
pub fn snapshot(c: &Case, extra: &[Pubkey]) -> (Vec<Pubkey>, Vec<Option<solana_account::Account>>) {
    let mut keys = c.protected(0);
    keys.extend(extra);
    let values = c.snapshot(&keys);
    (keys, values)
}
pub fn error(f: crate::TxResult, expected: &str) -> litesvm::types::FailedTransactionMetadata {
    let e = f.expect_err(expected);
    assert!(
        e.meta.logs.iter().any(|l| l.contains(expected)),
        "expected {expected}, {:?}\n{}",
        e.err,
        e.meta.logs.join("\n")
    );
    e
}
