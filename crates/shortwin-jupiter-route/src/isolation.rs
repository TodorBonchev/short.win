//! Conservative first-probe isolation checks, independent of route parsing.
//!
//! These functions inspect caller-supplied runtime snapshots. An integration must read
//! genuine AccountInfo fields, construct CPI metas deliberately, verify the pinned
//! executable, derive fresh scratch PDAs and enforce these checks before/after CPI.
//! Passing this module is not route, financial, mint or deployment authorization.
use crate::{AccountMeta, Address, ExpectedAccounts, SPL_TOKEN_PROGRAM, TOKEN_2022_PROGRAM};

/// The ordinary SPL Token native mint; Token-2022 native wrappers are outside this probe.
pub const WSOL_MINT: Address = [
    6, 155, 136, 87, 254, 171, 129, 132, 251, 104, 127, 99, 70, 24, 192, 53, 218, 196, 57, 220, 26,
    235, 59, 85, 152, 160, 240, 0, 0, 0, 0, 1,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolationError {
    InvalidBindings,
    AccountCount,
    RuntimeMismatch,
    DuplicateRuntimeMismatch,
    ForbiddenAccount,
    ProtocolOwnedAccount,
    UnexpectedSigner,
    WritableAuthority,
    WritableIdentity,
    InvalidScratchProgram,
    ExtendedScratchAccount,
    ScratchIdentity,
    ScratchState,
    ScratchAuthority,
    ScratchRent,
    InvalidFunding,
    BalanceDirection,
    InsufficientOutput,
}

/// Fields read from live runtime accounts, not API-declared roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeAccount {
    pub key: Address,
    pub owner: Address,
    pub is_signer: bool,
    pub is_writable: bool,
    pub executable: bool,
}

/// Restricts an ordinary ExactIn probe's CPI surface. Header/fixed-account checks
/// must run separately. Runtime privileges can exceed proposed metas because of
/// transaction-wide privilege union; the authority must remain read-only in both.
/// Every protocol-owned account and every named critical key is refused, even read-only.
pub fn check_surface(
    proposed: &[AccountMeta],
    runtime: &[RuntimeAccount],
    expected: &ExpectedAccounts,
    current_program: Address,
    forbidden: &[Address],
    maximum_accounts: usize,
) -> Result<(), IsolationError> {
    if current_program == [0; 32] || expected.shared.is_some() {
        return Err(IsolationError::InvalidBindings);
    }
    let roles = [
        expected.transfer_authority,
        expected.source_token_account,
        expected.destination_token_account,
        expected.source_mint,
        expected.destination_mint,
    ];
    for (i, key) in roles.iter().enumerate() {
        if *key == current_program
            || *key == [0; 32]
            || roles[..i].contains(key)
            || *key == expected.source_token_program
            || *key == expected.destination_token_program
        {
            return Err(IsolationError::InvalidBindings);
        }
    }
    if proposed.len() != runtime.len()
        || proposed.is_empty()
        || proposed.len() > maximum_accounts
        || maximum_accounts > 128
    {
        return Err(IsolationError::AccountCount);
    }
    for (i, (meta, actual)) in proposed.iter().zip(runtime).enumerate() {
        if meta.key != actual.key || (meta.is_writable && !actual.is_writable) {
            return Err(IsolationError::RuntimeMismatch);
        }
        // Solana aliases have one underlying owner/privilege set. Refuse a caller
        // snapshot that conceals an alias's union privileges behind another copy.
        for other in &runtime[..i] {
            if other.key == actual.key && *other != *actual {
                return Err(IsolationError::DuplicateRuntimeMismatch);
            }
        }
        if actual.key == current_program || forbidden.contains(&actual.key) {
            return Err(IsolationError::ForbiddenAccount);
        }
        if actual.owner == current_program {
            return Err(IsolationError::ProtocolOwnedAccount);
        }
        if (meta.is_signer || actual.is_signer) && meta.key != expected.transfer_authority {
            return Err(IsolationError::UnexpectedSigner);
        }
        if meta.key == expected.transfer_authority {
            if meta.is_writable || actual.is_writable {
                return Err(IsolationError::WritableAuthority);
            }
            if actual.owner != [0; 32] || actual.executable {
                return Err(IsolationError::InvalidBindings);
            }
        }
        if [
            expected.source_mint,
            expected.destination_mint,
            expected.source_token_program,
            expected.destination_token_program,
        ]
        .contains(&meta.key)
            && (meta.is_writable || actual.is_writable)
        {
            return Err(IsolationError::WritableIdentity);
        }
    }
    Ok(())
}

/// Trusted executor-created scratch identity and rent values. Exact plain 165-byte
/// accounts only, including Token-2022; no TLV tail or attacker-created ATA is accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScratchBinding {
    pub key: Address,
    pub token_program: Address,
    pub mint: Address,
    pub authority: Address,
    pub rent_lamports: u64,
    pub native: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScratchState {
    binding: ScratchBinding,
    amount: u64,
}
impl ScratchState {
    pub const fn amount(self) -> u64 {
        self.amount
    }
}

fn key(data: &[u8], start: usize) -> Address {
    let mut value = [0; 32];
    value.copy_from_slice(&data[start..start + 32]);
    value
}
fn u64_at(data: &[u8], start: usize) -> u64 {
    let mut value = [0; 8];
    value.copy_from_slice(&data[start..start + 8]);
    u64::from_le_bytes(value)
}

/// Checks immutable identities, initialized state, no delegation/close authority,
/// exact rent and native balance consistency. Reading base fields while ignoring
/// Token-2022 extensions is deliberately insufficient for this first probe.
pub fn read_plain_scratch(
    actual: RuntimeAccount,
    data: &[u8],
    lamports: u64,
    binding: ScratchBinding,
) -> Result<ScratchState, IsolationError> {
    if ![SPL_TOKEN_PROGRAM, TOKEN_2022_PROGRAM].contains(&binding.token_program)
        || actual.owner != binding.token_program
        || actual.executable
        || (binding.native
            && (binding.token_program != SPL_TOKEN_PROGRAM || binding.mint != WSOL_MINT))
    {
        return Err(IsolationError::InvalidScratchProgram);
    }
    if data.len() != 165 {
        return Err(IsolationError::ExtendedScratchAccount);
    }
    if actual.key != binding.key
        || key(data, 0) != binding.mint
        || key(data, 32) != binding.authority
    {
        return Err(IsolationError::ScratchIdentity);
    }
    if data[108] != 1 {
        return Err(IsolationError::ScratchState);
    }
    if data[72..76] != [0; 4] || data[129..133] != [0; 4] || u64_at(data, 121) != 0 {
        return Err(IsolationError::ScratchAuthority);
    }
    let amount = u64_at(data, 64);
    let expected_lamports = if binding.native {
        if data[109..113] != [1, 0, 0, 0] || u64_at(data, 113) != binding.rent_lamports {
            return Err(IsolationError::ScratchRent);
        }
        binding
            .rent_lamports
            .checked_add(amount)
            .ok_or(IsolationError::ScratchRent)?
    } else {
        if data[109..113] != [0; 4] {
            return Err(IsolationError::ScratchRent);
        }
        binding.rent_lamports
    };
    if binding.rent_lamports == 0 || lamports != expected_lamports {
        return Err(IsolationError::ScratchRent);
    }
    Ok(ScratchState { binding, amount })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuybackDeltas {
    pub spent_lamports: u64,
    pub received_tokens: u64,
}

/// Whole-debt output on isolated scratch, not yet final pool credit or debt release.
/// The integration must transfer all output into the checked pool vault, verify its
/// net delta, refund remaining WSOL/rent, check native escrow all-in debit and only
/// then apply the existing actual-cost waterfall/accounting atomically.
pub fn measure_buyback(
    source_before: ScratchState,
    source_after: ScratchState,
    destination_before: ScratchState,
    destination_after: ScratchState,
    maximum_input: u64,
    required_output: u64,
) -> Result<BuybackDeltas, IsolationError> {
    if source_before.binding != source_after.binding
        || destination_before.binding != destination_after.binding
        || source_before.binding.key == destination_before.binding.key
        || source_before.binding.mint == destination_before.binding.mint
        || source_before.binding.authority != destination_before.binding.authority
        || !source_before.binding.native
        || destination_before.binding.native
    {
        return Err(IsolationError::InvalidBindings);
    }
    if maximum_input == 0
        || required_output == 0
        || source_before.amount == 0
        || source_before.amount > maximum_input
        || destination_before.amount != 0
    {
        return Err(IsolationError::InvalidFunding);
    }
    let spent_lamports = source_before
        .amount
        .checked_sub(source_after.amount)
        .ok_or(IsolationError::BalanceDirection)?;
    let received_tokens = destination_after
        .amount
        .checked_sub(destination_before.amount)
        .ok_or(IsolationError::BalanceDirection)?;
    if received_tokens < required_output {
        return Err(IsolationError::InsufficientOutput);
    }
    Ok(BuybackDeltas {
        spent_lamports,
        received_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    fn binding(native: bool) -> ScratchBinding {
        ScratchBinding {
            key: if native { [2; 32] } else { [3; 32] },
            token_program: SPL_TOKEN_PROGRAM,
            mint: if native { WSOL_MINT } else { [5; 32] },
            authority: [1; 32],
            rent_lamports: 100,
            native,
        }
    }
    fn token(b: ScratchBinding, amount: u64) -> (RuntimeAccount, [u8; 165], u64) {
        let mut data = [0; 165];
        data[..32].copy_from_slice(&b.mint);
        data[32..64].copy_from_slice(&b.authority);
        data[64..72].copy_from_slice(&amount.to_le_bytes());
        data[108] = 1;
        if b.native {
            data[109] = 1;
            data[113..121].copy_from_slice(&b.rent_lamports.to_le_bytes());
        }
        (
            RuntimeAccount {
                key: b.key,
                owner: b.token_program,
                is_signer: false,
                is_writable: true,
                executable: false,
            },
            data,
            b.rent_lamports + if b.native { amount } else { 0 },
        )
    }
    fn state(b: ScratchBinding, n: u64) -> ScratchState {
        let (a, d, l) = token(b, n);
        read_plain_scratch(a, &d, l, b).unwrap()
    }
    fn expected() -> ExpectedAccounts {
        ExpectedAccounts {
            transfer_authority: [1; 32],
            source_token_account: [2; 32],
            destination_token_account: [3; 32],
            source_mint: [4; 32],
            destination_mint: [5; 32],
            source_token_program: SPL_TOKEN_PROGRAM,
            destination_token_program: TOKEN_2022_PROGRAM,
            shared: None,
        }
    }
    fn surface() -> (std::vec::Vec<AccountMeta>, std::vec::Vec<RuntimeAccount>) {
        let metas = vec![
            AccountMeta {
                key: [1; 32],
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                key: [2; 32],
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                key: [8; 32],
                is_signer: false,
                is_writable: true,
            },
        ];
        let actual = metas
            .iter()
            .map(|m| RuntimeAccount {
                key: m.key,
                owner: [0; 32],
                is_signer: false,
                is_writable: m.is_writable,
                executable: false,
            })
            .collect();
        (metas, actual)
    }
    #[test]
    fn rejects_runtime_union_and_alias_privileges() {
        let (mut m, mut r) = surface();
        assert_eq!(check_surface(&m, &r, &expected(), [9; 32], &[], 64), Ok(()));
        r[0].is_writable = true;
        assert_eq!(
            check_surface(&m, &r, &expected(), [9; 32], &[], 64),
            Err(IsolationError::WritableAuthority)
        );
        r[0].is_writable = false;
        m.push(m[0]);
        r.push(r[0]);
        r[3].is_writable = true;
        assert_eq!(
            check_surface(&m, &r, &expected(), [9; 32], &[], 64),
            Err(IsolationError::DuplicateRuntimeMismatch)
        );
    }
    #[test]
    fn refuses_protocol_state_keys_unexpected_signers_and_writable_mints() {
        let (mut m, mut r) = surface();
        r[2].owner = [9; 32];
        assert_eq!(
            check_surface(&m, &r, &expected(), [9; 32], &[], 64),
            Err(IsolationError::ProtocolOwnedAccount)
        );
        r[2].owner = [0; 32];
        assert_eq!(
            check_surface(&m, &r, &expected(), [9; 32], &[[8; 32]], 64),
            Err(IsolationError::ForbiddenAccount)
        );
        r[2].is_signer = true;
        assert_eq!(
            check_surface(&m, &r, &expected(), [9; 32], &[], 64),
            Err(IsolationError::UnexpectedSigner)
        );
        r[2].is_signer = false;
        m[2].key = [4; 32];
        r[2].key = [4; 32];
        assert_eq!(
            check_surface(&m, &r, &expected(), [9; 32], &[], 64),
            Err(IsolationError::WritableIdentity)
        );
    }
    #[test]
    fn refuses_hidden_extensions_state_and_authority_changes() {
        let b = binding(false);
        let (a, d, l) = token(b, 4);
        assert!(read_plain_scratch(a, &d, l, b).is_ok());
        let mut extended = d.to_vec();
        extended.push(2);
        assert_eq!(
            read_plain_scratch(a, &extended, l, b),
            Err(IsolationError::ExtendedScratchAccount)
        );
        for at in [72, 108, 129] {
            let mut bad = d;
            bad[at] = 2;
            assert!(read_plain_scratch(a, &bad, l, b).is_err());
        }
        let mut bad = d;
        bad[32] = 77;
        assert_eq!(
            read_plain_scratch(a, &bad, l, b),
            Err(IsolationError::ScratchIdentity)
        );
        let mut a = a;
        a.owner = TOKEN_2022_PROGRAM;
        assert_eq!(
            read_plain_scratch(a, &d, l, b),
            Err(IsolationError::InvalidScratchProgram)
        );
    }
    #[test]
    fn native_rent_lamports_and_delegated_amount_are_checked() {
        let b = binding(true);
        let (a, d, l) = token(b, 4);
        for at in [109, 113, 121] {
            let mut bad = d;
            bad[at] = 3;
            assert!(read_plain_scratch(a, &bad, l, b).is_err());
        }
        assert_eq!(
            read_plain_scratch(a, &d, l + 1, b),
            Err(IsolationError::ScratchRent)
        );
        let mut b = b;
        b.rent_lamports = u64::MAX;
        assert_eq!(
            read_plain_scratch(a, &d, l, b),
            Err(IsolationError::ScratchRent)
        );
    }
    #[test]
    fn whole_debt_net_credit_bounded_funding_and_surplus() {
        let s = binding(true);
        let d = binding(false);
        let run = |n| measure_buyback(state(s, 80), state(s, 10), state(d, 0), state(d, n), 80, 50);
        assert_eq!(
            run(50),
            Ok(BuybackDeltas {
                spent_lamports: 70,
                received_tokens: 50
            })
        );
        assert_eq!(run(60).unwrap().received_tokens, 60);
        assert_eq!(run(49), Err(IsolationError::InsufficientOutput));
        assert_eq!(
            measure_buyback(state(s, 81), state(s, 0), state(d, 0), state(d, 50), 80, 50),
            Err(IsolationError::InvalidFunding)
        );
        assert_eq!(
            measure_buyback(
                state(s, 80),
                state(s, 81),
                state(d, 0),
                state(d, 50),
                80,
                50
            ),
            Err(IsolationError::BalanceDirection)
        );
        assert_eq!(
            measure_buyback(state(s, 80), state(s, 0), state(d, 1), state(d, 50), 80, 50),
            Err(IsolationError::InvalidFunding)
        );
    }
}
