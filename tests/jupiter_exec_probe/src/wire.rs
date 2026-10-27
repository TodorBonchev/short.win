//! TEST-ONLY ABI shared by the isolated SBF binary and its LiteSVM tests.
//! Values are synthetic; no mainnet address or production financial policy is selected.
pub const EXECUTOR_ID: [u8; 32] = [0xd1; 32];
pub const HOSTILE_TARGET_ID: [u8; 32] = [0xd2; 32];
pub const ESCROW_SEED: &[u8] = b"probe-escrow";
pub const AUTHORITY_SEED: &[u8] = b"probe-authority";
pub const INPUT_SEED: &[u8] = b"probe-input";
pub const OUTPUT_SEED: &[u8] = b"probe-output";
pub const POOL_SEED: &[u8] = b"probe-pool";
pub const POOL_OWNER_SEED: &[u8] = b"probe-pool-owner";
pub const TARGET_AUTHORITY_SEED: &[u8] = b"probe-target";
pub const VERSION: u8 = 1;
pub const FIXED_ACCOUNTS: usize = 14;
pub const MAXIMUM_ACCOUNTS: usize = 32;
pub const PINNED_ROUTE_ACCOUNTS: usize = 19;
pub const FLAG_PINNED_JUPITER: u8 = 2;
pub const MODE_GOOD: u8 = 0;
pub const MODE_DELEGATE: u8 = 1;
pub const MODE_OWNER: u8 = 2;
pub const MODE_EARLY_CLOSE: u8 = 3;
pub const MODE_STEAL_NATIVE: u8 = 4;
pub const MODE_REENTER: u8 = 5;
pub const FORCE_POST_FAILURE: u8 = 1;

pub const ERR_INSTRUCTION: u32 = 7000;
pub const ERR_ACCOUNTS: u32 = 7001;
pub const ERR_BINDINGS: u32 = 7002;
pub const ERR_FUNDING: u32 = 7003;
pub const ERR_SURFACE: u32 = 7004;
pub const ERR_SCRATCH: u32 = 7005;
pub const ERR_DELIVERY: u32 = 7006;
pub const ERR_POOL_CREDIT: u32 = 7007;
pub const ERR_REFUND: u32 = 7008;
pub const ERR_FORCED_POST: u32 = 7009;
pub const ERR_TARGET: u32 = 7010;
pub const ERR_MINT: u32 = 7011;

/// Whole-debt BuyAtLeast test bounds. No financial authority is inferred from these bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub mode: u8,
    pub flags: u8,
    pub quantity: u64,
    pub input_amount: u64,
    pub maximum_input: u64,
    pub target_spend: u64,
    pub target_delivery: u64,
}
impl Request {
    pub const LEN: usize = 43;
    pub fn encode(self) -> [u8; Self::LEN] {
        let mut out = [0; Self::LEN];
        out[0] = VERSION;
        out[1] = self.mode;
        out[2] = self.flags;
        for (i, value) in [
            self.quantity,
            self.input_amount,
            self.maximum_input,
            self.target_spend,
            self.target_delivery,
        ]
        .iter()
        .enumerate()
        {
            out[3 + i * 8..11 + i * 8].copy_from_slice(&value.to_le_bytes());
        }
        out
    }
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != Self::LEN
            || data[0] != VERSION
            || data[1] > MODE_REENTER
            || data[2] & !(FORCE_POST_FAILURE | FLAG_PINNED_JUPITER) != 0
        {
            return None;
        }
        let mut values = [0; 5];
        for (i, value) in values.iter_mut().enumerate() {
            let mut raw = [0; 8];
            raw.copy_from_slice(&data[3 + i * 8..11 + i * 8]);
            *value = u64::from_le_bytes(raw);
        }
        Some(Self {
            mode: data[1],
            flags: data[2],
            quantity: values[0],
            input_amount: values[1],
            maximum_input: values[2],
            target_spend: values[3],
            target_delivery: values[4],
        })
    }
}
pub const PINNED_CPMM_PROGRAM: [u8; 32] = [
    169, 42, 90, 139, 79, 41, 89, 82, 132, 37, 80, 170, 147, 253, 91, 149, 181, 172, 230, 168, 235,
    146, 12, 147, 148, 46, 67, 105, 12, 32, 236, 115,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_roundtrip_and_strict_lengths() {
        let x = Request {
            mode: MODE_GOOD,
            flags: 0,
            quantity: u64::MAX,
            input_amount: 1,
            maximum_input: 2,
            target_spend: 3,
            target_delivery: 4,
        };
        assert_eq!(Request::decode(&x.encode()), Some(x));
        for n in 0..Request::LEN {
            assert!(Request::decode(&x.encode()[..n]).is_none());
        }
        let mut extra = x.encode().to_vec();
        extra.push(0);
        assert!(Request::decode(&extra).is_none());
        let mut wrong = x.encode();
        wrong[2] = 4;
        assert!(Request::decode(&wrong).is_none());
    }
}
