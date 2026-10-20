# shortwin

The [short.win](https://github.com/TodorBonchev/short.win) Anchor program (Anchor 1.2): permissionless peer-to-peer token lending
for shorting Solana memecoins. Use this crate with the `cpi` feature to invoke short.win
from your own on-chain program, or with `no-entrypoint` to use its account types,
instruction definitions, events and error codes off-chain.

> **Not deployed yet.** The declared program id
> `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` (`shortwin::ID`) is not live on any
> cluster today. Until it is, CPI calls against it will fail; test against your own
> deployment on a local validator. The interface may change before the mainnet launch.

## CPI usage

```toml
[dependencies]
shortwin = { version = "1.0.2", features = ["cpi"] }
anchor-lang = "=1.2.0"
```

```rust,ignore
use anchor_lang::prelude::*;
use shortwin::cpi::{accounts::AddMargin, add_margin};

pub fn top_up<'info>(
    shortwin_program: AccountInfo<'info>,
    accounts: AddMargin<'info>,
    amount: u64,
) -> Result<()> {
    // shortwin_program must be the short.win program (key == shortwin::ID).
    add_margin(CpiContext::new(shortwin_program, accounts), amount)
}
```

Every instruction has a `shortwin::cpi::<name>` function and a
`shortwin::cpi::accounts::<Name>` account struct. Account state lives in
`shortwin::state`, events in `shortwin::events` and error codes in `shortwin::errors`.
The pure protocol math is [`shortwin-math`](https://crates.io/crates/shortwin-math) and the
venue account readers are [`shortwin-venue-state`](https://crates.io/crates/shortwin-venue-state).
For TypeScript, use the npm package [`@shortwin/client`](https://www.npmjs.com/package/@shortwin/client).

## Features

| Feature | Effect |
|---|---|
| `cpi` | CPI helpers; implies `no-entrypoint` |
| `no-entrypoint` | build as a library without the program entrypoint |
| `idl-build` | Anchor IDL generation |
| `test-venue` | TEST-ONLY mock venue; never in a release build |

Build the deployable program with `anchor build --tools-version v1.57 --arch v3` from the
repository (see docs/DEVELOPMENT.md), not from this crate.

## Licence

Business Source License 1.1 (`LICENSE` in this package; Change Date 2030-10-01, Change
License Apache-2.0). Production use is permitted solely to interact with the Official
Deployment, as set out in the Additional Use Grant and in
[GRANTS.md](https://github.com/TodorBonchev/short.win/blob/main/GRANTS.md).
