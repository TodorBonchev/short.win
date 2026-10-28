//! Historical, hash-checked Jupiter/CPMM route witness for actual-SBF tests only.
//! No product pricing, route admission or direct DEX execution depends on this helper.
use anchor_lang::prelude::{pubkey, Pubkey};
use litesvm::LiteSVM;
use serde_json::Value;
use sha2::{Digest, Sha256};
use shortwin_simple::route::{JUPITER_ID, JUPITER_PROGRAM_DATA, TOKEN_ID, WSOL_MINT};
use solana_account::Account;
use solana_instruction::AccountMeta;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

pub const JUPITER_SLOT: u64 = 459_479_624;
pub const EXECUTION_SLOT: u64 = 461_997_165;
pub const HISTORICAL_BLOCK_TIME: i64 = 1_790_563_273;
pub const JUPITER_PROGRAMDATA_LEN: u32 = 2_892_269;
pub const JUPITER_ELF_SHA: &str =
    "5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b";
pub const CPMM_ID: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
pub const EVENT_AUTHORITY: Pubkey = pubkey!("D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf");

fn key(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn checked(path: &Path, expected: &str) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        hex(&Sha256::digest(&bytes)),
        expected,
        "fixture hash {}",
        path.display()
    );
    bytes
}
pub fn repository() -> PathBuf {
    std::env::var_os("SHORTWIN_SIMPLE_FIXTURE_ROOT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."),
        PathBuf::from,
    )
}
#[derive(Clone, Debug)]
pub struct HistoricalRoute {
    pub base_mint: Pubkey,
    pub base_decimals: u8,
    pub pool: Pubkey,
    pub config: Pubkey,
    pub observation: Pubkey,
    pub authority: Pubkey,
    pub wsol_vault: Pubkey,
    pub base_vault: Pubkey,
    /// All actual DEX accounts which a rollback assertion should compare.
    pub protected: Vec<Pubkey>,
}
impl HistoricalRoute {
    /// Ordinary Router V2 exact-in, one historical RaydiumCPMM step. Only the
    /// scratch authority is marked signer in the CPI; never an outer signature.
    pub fn accounts(
        &self,
        authority: Pubkey,
        base: Pubkey,
        wsol: Pubkey,
        buy: bool,
    ) -> Vec<AccountMeta> {
        let (source, destination, input_mint, output_mint, input_vault, output_vault) = if buy {
            (
                wsol,
                base,
                WSOL_MINT,
                self.base_mint,
                self.wsol_vault,
                self.base_vault,
            )
        } else {
            (
                base,
                wsol,
                self.base_mint,
                WSOL_MINT,
                self.base_vault,
                self.wsol_vault,
            )
        };
        let ro = |k| AccountMeta::new_readonly(k, false);
        let rw = |k| AccountMeta::new(k, false);
        vec![
            ro(authority),
            rw(source),
            rw(destination),
            ro(input_mint),
            ro(output_mint),
            ro(TOKEN_ID),
            ro(TOKEN_ID),
            ro(JUPITER_ID),
            ro(EVENT_AUTHORITY),
            ro(JUPITER_ID),
            ro(CPMM_ID),
            ro(authority),
            ro(self.authority),
            ro(self.config),
            rw(self.pool),
            rw(source),
            rw(destination),
            rw(input_vault),
            rw(output_vault),
            ro(TOKEN_ID),
            ro(TOKEN_ID),
            ro(input_mint),
            ro(output_mint),
            rw(self.observation),
        ]
    }
    pub fn data(input: u64, minimum_output: u64) -> Vec<u8> {
        let mut out = vec![187, 100, 250, 204, 49, 196, 175, 20]; // route_v2, on-chain frozen IDL
        out.extend(input.to_le_bytes());
        out.extend(minimum_output.to_le_bytes());
        out.extend([0; 6]);
        out.extend(1u32.to_le_bytes());
        out.extend([46, 0x10, 0x27, 0, 1]); // RaydiumCPMM, 10000 bps, token0 -> token1
        assert_eq!(out.len(), 39);
        out
    }
    pub fn reviewed() -> shortwin_simple::route::ReviewedDeployment {
        let mut digest = [0; 32];
        for (i, x) in digest.iter_mut().enumerate() {
            *x = u8::from_str_radix(&JUPITER_ELF_SHA[i * 2..i * 2 + 2], 16).unwrap();
        }
        shortwin_simple::route::ReviewedDeployment {
            deploy_slot: JUPITER_SLOT,
            programdata_len: JUPITER_PROGRAMDATA_LEN,
            elf_sha256: digest,
        }
    }
}
/// Install frozen fixture bytes, verifying the original tracked hashes before
/// LiteSVM compiles them. Materialize Jupiter with scripts/materialize-jupiter-probe.py.
pub fn install(svm: &mut LiteSVM) -> HistoricalRoute {
    let root = repository();
    let fixtures = root.join("tests/fixtures");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("manifest.json")).unwrap()).unwrap();
    let entries = manifest["accounts"].as_array().unwrap();
    let entry = |name: &str| entries.iter().find(|v| v["name"] == name).unwrap().clone();
    let account = |v: &Value, data: Vec<u8>| Account {
        lamports: v["lamports"].as_u64().unwrap(),
        data,
        owner: key(v["owner"].as_str().unwrap()),
        executable: v["executable"].as_bool().unwrap(),
        rent_epoch: v["rent_epoch"].as_u64().unwrap(),
    };
    let mut protected = vec![];
    for name in [
        "mint_wsol",
        "cpmm_authority",
        "cpmm_config_00",
        "cpmm_useless_pool",
        "cpmm_useless_token_0_vault",
        "cpmm_useless_token_1_vault",
        "cpmm_useless_token_mint",
        "cpmm_useless_observation",
    ] {
        let v = entry(name);
        let address = key(v["address"].as_str().unwrap());
        let data = checked(
            &fixtures.join(v["file"].as_str().unwrap()),
            v["sha256"].as_str().unwrap(),
        );
        svm.set_account(address, account(&v, data)).unwrap();
        protected.push(address);
    }
    // The known October CPMM ELF is only this historical route's inner target.
    for name in [
        "raydium_cpmm_2026_10_program_data",
        "raydium_cpmm_2026_10_program",
    ] {
        let v = entry(name);
        let data = if v["file"].is_string() {
            checked(
                &fixtures.join(v["file"].as_str().unwrap()),
                v["sha256"].as_str().unwrap(),
            )
        } else {
            let mut d =
                std::fs::read(fixtures.join(v["files"]["header"].as_str().unwrap())).unwrap();
            d.extend(checked(
                &fixtures.join(v["files"]["elf"].as_str().unwrap()),
                v["elf_sha256"].as_str().unwrap(),
            ));
            d.resize(v["space"].as_u64().unwrap() as usize, 0);
            assert_eq!(hex(&Sha256::digest(&d)), v["sha256"].as_str().unwrap());
            d
        };
        svm.set_account(key(v["address"].as_str().unwrap()), account(&v, data))
            .unwrap();
    }
    let candidate = std::env::var_os("SHORTWIN_JUPITER_CANDIDATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/jupiter-exec-probe/jupiter-candidate"));
    let pd = checked(
        &candidate.join("programdata.bin"),
        "21e28c59e3b0a5536e48d31a1ca7346514a2bc61d70dc1388c920d7e3be35c1b",
    );
    assert_eq!(pd.len(), JUPITER_PROGRAMDATA_LEN as usize);
    assert_eq!(
        u64::from_le_bytes(pd[4..12].try_into().unwrap()),
        JUPITER_SLOT
    );
    let trimmed = pd[45..].iter().rposition(|b| *b != 0).unwrap() + 46;
    assert_eq!(hex(&Sha256::digest(&pd[45..trimmed])), JUPITER_ELF_SHA);
    let program = checked(
        &candidate.join("program.bin"),
        "77969e6c064ab31f68250951941ad20690dfc23e97870e63006ccdad39c8a47e",
    );
    assert_eq!(&program[4..], JUPITER_PROGRAM_DATA.as_ref());
    let loader = anchor_lang::solana_program::bpf_loader_upgradeable::ID;
    for (address, data, executable) in [
        (JUPITER_PROGRAM_DATA, pd, false),
        (JUPITER_ID, program, true),
    ] {
        svm.set_account(
            address,
            Account {
                lamports: 30_000_000_000,
                data,
                owner: loader,
                executable,
                rent_epoch: u64::MAX,
            },
        )
        .unwrap();
    }
    svm.set_account(
        EVENT_AUTHORITY,
        Account {
            lamports: 1,
            ..Account::default()
        },
    )
    .unwrap();
    let p = entry("cpmm_useless_pool");
    let d = &p["decoded"];
    HistoricalRoute {
        base_mint: key(d["token_1_mint"].as_str().unwrap()),
        base_decimals: 6,
        pool: key(p["address"].as_str().unwrap()),
        config: key(d["amm_config"].as_str().unwrap()),
        observation: key(d["observation_key"].as_str().unwrap()),
        authority: key(entry("cpmm_authority")["address"].as_str().unwrap()),
        wsol_vault: key(d["token_0_vault"].as_str().unwrap()),
        base_vault: key(d["token_1_vault"].as_str().unwrap()),
        protected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_real_elf_snapshot_loads_without_the_historical_product() {
        let mut svm = LiteSVM::new();
        let witness = install(&mut svm);
        assert!(svm.get_account(&JUPITER_ID).unwrap().executable);
        assert!(svm.get_account(&CPMM_ID).unwrap().executable);
        assert_eq!(svm.get_account(&witness.base_mint).unwrap().data.len(), 82);
        assert_eq!(
            witness
                .accounts(
                    Pubkey::new_unique(),
                    Pubkey::new_unique(),
                    Pubkey::new_unique(),
                    true
                )
                .len(),
            24
        );
    }
}
