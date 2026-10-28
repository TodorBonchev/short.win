//! Genuine new-program -> frozen Jupiter Router V2 -> frozen CPMM opening.
//! Historical mint/pool state and a synthetic initial lender allocation are local
//! fixtures. The v0 ALT is test-only. No live price provenance/landing is claimed.
use crate::{
    assert_ok, deterministic_key, env as pda,
    jupiter::{self, HistoricalRoute},
    tokens::{self, TokenProgram},
    Env,
};
use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use shortwin_simple::{
    accounts, instruction,
    open::OpenArgs,
    reference::PublishReferenceArgs,
    route::{self, RouteArgs},
};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;
pub const Q: u64 = 1_000_000;
pub fn instruction(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
    Instruction {
        program_id: shortwin_simple::ID,
        accounts: accounts.to_account_metas(None),
        data: data.data(),
    }
}
pub fn loan(pool: Pubkey, borrower: Pubkey, nonce: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"loan",
            pool.as_ref(),
            borrower.as_ref(),
            &nonce.to_le_bytes(),
        ],
        &shortwin_simple::ID,
    )
    .0
}
pub fn exposure(pool: Pubkey, borrower: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"exposure", pool.as_ref(), borrower.as_ref()],
        &shortwin_simple::ID,
    )
    .0
}
pub fn reference(market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"reference",
            market.as_ref(),
            pda::policy(1).as_ref(),
            &Q.to_le_bytes(),
        ],
        &shortwin_simple::ID,
    )
    .0
}
pub struct PositionCase {
    pub env: Env,
    pub route: HistoricalRoute,
    pub borrower: Keypair,
    pub lender: Keypair,
    pub market: Pubkey,
    pub pool: Pubkey,
    pub reference: Pubkey,
}
impl PositionCase {
    pub fn new() -> Self {
        let mut env = Env::new();
        let route = jupiter::install(&mut env.svm);
        let args = env.initialize_args();
        assert_ok(env.initialize(args));
        let borrower = deterministic_key(100);
        let lender = deterministic_key(101);
        env.fund(borrower.pubkey(), 10_000_000_000);
        env.fund(lender.pubkey(), 1_000_000_000);
        assert_ok(env.register_market(
            &lender,
            route.base_mint,
            TokenProgram::Spl.id(),
            10_000,
            100_000_000,
        ));
        let market = pda::market(&route.base_mint);
        let pool = pda::pool(&market, 0);
        let source = deterministic_key(102);
        let source = tokens::create_account(
            &mut env,
            &source,
            &route.base_mint,
            &lender.pubkey(),
            TokenProgram::Spl,
        );
        // Synthetic initial allocation from the historical mint's existing supply.
        // All subsequent deposits/sales execute the real token and router programs.
        let mut a = env.svm.get_account(&source).unwrap();
        a.data[64..72].copy_from_slice(&(10 * Q).to_le_bytes());
        env.svm.set_account(source, a).unwrap();
        let accounts = accounts::DepositTokens {
            owner: lender.pubkey(),
            market,
            pool,
            receipt: pda::lender(&pool, &lender.pubkey()),
            mint: route.base_mint,
            owner_tokens: source,
            token_vault: pda::token_vault(&pool),
            token_program: TokenProgram::Spl.id(),
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        };
        assert_ok(env.send(
            &[instruction(
                accounts,
                instruction::Deposit {
                    amount: 10 * Q,
                    min_shares: u128::from(10 * Q),
                    max_receipt_rent: 100_000_000,
                },
            )],
            &[&lender],
        ));
        let reference = reference(market);
        let timestamp = pda::START_TS;
        let epoch = timestamp / 60 * 60;
        let args = PublishReferenceArgs {
            quantity: Q,
            sequence: 1,
            source_slot: jupiter::EXECUTION_SLOT,
            source_ts: timestamp,
            finite: true,
            live_bid_lower: 2_200_000,
            live_ask_upper: 2_400_000,
            history_ask_upper_max: 2_400_000,
            history_bid_lower_max: 2_200_000,
            acquisition_reference: 2_400_000,
            completed_epochs: 2,
            history_start_ts: epoch - 120,
            history_end_ts: epoch - 1,
            evidence_hash: [9; 32],
            max_reference_rent: 100_000_000,
        };
        let reporter = env.reporter.insecure_clone();
        let accounts = accounts::PublishReference {
            reporter: reporter.pubkey(),
            config: pda::config(),
            market,
            policy: pda::policy(1),
            reference,
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        };
        assert_ok(env.send(
            &[instruction(
                accounts,
                instruction::PublishReference { args },
            )],
            &[&reporter],
        ));
        Self {
            env,
            route,
            borrower,
            lender,
            market,
            pool,
            reference,
        }
    }
    pub fn args(&self) -> OpenArgs {
        OpenArgs {
            nonce: 0,
            q: Q,
            min_proceeds: 1,
            max_margin: 10_000_000,
            max_term_fee: 1_000_000,
            temporary_funding: 100_000_000,
            max_account_rent: 100_000_000,
            minimum_reference_sequence: 1,
            route: RouteArgs {
                input_amount: Q,
                data: HistoricalRoute::data(Q, 1),
            },
        }
    }
    pub fn ix(&self, args: OpenArgs) -> Instruction {
        let loan = loan(self.pool, self.borrower.pubkey(), args.nonce);
        let derive = |seed: &[u8]| {
            Pubkey::find_program_address(&[seed, loan.as_ref()], &shortwin_simple::ID).0
        };
        let authority = derive(route::AUTHORITY_SEED);
        let base = derive(route::BASE_SEED);
        let wsol = derive(route::WSOL_SEED);
        let accounts = accounts::OpenPosition {
            borrower: self.borrower.pubkey(),
            config: pda::config(),
            policy: pda::policy(1),
            market: self.market,
            pool: self.pool,
            loan,
            exposure: exposure(self.pool, self.borrower.pubkey()),
            reference: self.reference,
            mint: self.route.base_mint,
            token_vault: pda::token_vault(&self.pool),
            token_program: route::TOKEN_ID,
            fee_recipient: self.env.fee_recipient.pubkey(),
            execution_authority: authority,
            execution_base: base,
            execution_wsol: wsol,
            wsol_mint: route::WSOL_MINT,
            wsol_token_program: route::TOKEN_ID,
            jupiter: route::JUPITER_ID,
            jupiter_program_data: route::JUPITER_PROGRAM_DATA,
            system_program: anchor_lang::system_program::ID,
            event_authority: pda::event_authority(),
            program: shortwin_simple::ID,
        };
        let mut ix = instruction(accounts, instruction::OpenPosition { args });
        ix.accounts
            .extend(self.route.accounts(authority, base, wsol, false));
        ix
    }
    pub fn protected(&self, nonce: u64) -> Vec<Pubkey> {
        let mut keys = self.route.protected.clone();
        let loan = loan(self.pool, self.borrower.pubkey(), nonce);
        keys.extend([
            self.market,
            self.pool,
            pda::token_vault(&self.pool),
            self.borrower.pubkey(),
            self.env.fee_recipient.pubkey(),
            exposure(self.pool, self.borrower.pubkey()),
            self.reference,
            loan,
        ]);
        for seed in [route::AUTHORITY_SEED, route::BASE_SEED, route::WSOL_SEED] {
            keys.push(Pubkey::find_program_address(&[seed, loan.as_ref()], &shortwin_simple::ID).0);
        }
        keys
    }
    pub fn snapshot(&self, keys: &[Pubkey]) -> Vec<Option<solana_account::Account>> {
        keys.iter().map(|k| self.env.svm.get_account(k)).collect()
    }
}

impl Default for PositionCase {
    fn default() -> Self {
        Self::new()
    }
}
