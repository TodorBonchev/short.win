//! Cross-language golden bytes, regenerated only deliberately and checked in.
use anchor_lang::{AccountSerialize, InstructionData};
use shortwin_simple::{open::OpenArgs, route::RouteArgs, state::*};
fn hex(x: &[u8]) -> String {
    x.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn rust_client_wire_contract() {
    let pool = PoolGeneration {
        version: 1,
        bump: 254,
        token_vault_bump: 253,
        market: shortwin_simple::ID,
        mint: shortwin_simple::route::WSOL_MINT,
        token_program: shortwin_simple::route::TOKEN_ID,
        generation: 3,
        cash: 9007199254740993,
        principal_out: 21,
        shares: (1u128 << 80) + 7,
        loan_count: 2,
        sol_reserve: 91,
        sol_index: [1, 2, 3, 4],
        retired: false,
        booked_notional_out: 100,
    };
    let mut pool_bytes = Vec::new();
    pool.try_serialize(&mut pool_bytes).unwrap();
    let open = shortwin_simple::instruction::OpenPosition {
        args: OpenArgs {
            nonce: 9007199254740995,
            q: 17,
            min_proceeds: 19,
            max_margin: 23,
            max_term_fee: 29,
            temporary_funding: 31,
            max_account_rent: 37,
            minimum_reference_sequence: 41,
            route: RouteArgs {
                input_amount: 17,
                setup_lamports: 3_123_456,
                data: vec![1, 2, 3, 4, 5, 6, 7, 8],
            },
        },
    }
    .data();
    let close = shortwin_simple::instruction::ClosePosition {
        args: shortwin_simple::exits::CloseArgs {
            max_buyback_cost: 101,
            temporary_funding: 103,
            route: RouteArgs {
                input_amount: 97,
                setup_lamports: 4_567_890,
                data: vec![8, 7, 6, 5, 4, 3, 2, 1],
            },
        },
    }
    .data();
    let value = serde_json::json!({"program":shortwin_simple::ID.to_string(),"pool":hex(&pool_bytes),"open":hex(&open),"close":hex(&close)});
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../client/test/fixtures/rust-wire.json");
    if std::env::var_os("UPDATE_SIMPLE_WIRE_FIXTURE").is_some() {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, serde_json::to_string_pretty(&value).unwrap() + "\n").unwrap();
    }
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(
        value, stored,
        "Rust wire contract changed; regenerate client/fixture together"
    );
}
