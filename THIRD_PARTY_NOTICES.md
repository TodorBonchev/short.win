# Third-party notices

[REUSE.toml](REUSE.toml) assigns file licenses; texts are in [LICENSES](LICENSES/). Cargo and pnpm dependencies are downloaded separately and retain their upstream licenses. The repository's own [LICENSE](LICENSE) and [GRANTS.md](GRANTS.md) remain binding.

## Jupiter and captured route state

`tests/fixtures/jupiter-2026-10-27` retains the pinned Jupiter Router V2 on-chain account response and decoded IDL. `tests/fixtures/current-routes` contains saved Jupiter build responses and the account/program snapshots needed by the maintained route matrix. These are interoperability test inputs. Runtime-byte identity is verified; Jupiter's private source build is not reproduced.

Captured route replies may contain deployed DEX or token-program code inside account data. A public-chain snapshot is not a grant of rights in that program. Upstream rights remain with the respective owners; no first-party license in this repository covers third-party embedded program code. Pump/PumpSwap programs have no published source license; retain the [pump.fun notice](LICENSES/LicenseRef-pump-fun-no-license.txt) when handling those snapshots.

## CPMM runtime fixture

`tests/fixtures/programs/raydium_cpmm_2026_10.so` is the pinned Raydium CPMM deployment at slot 452376460, from [raydium-io/raydium-cp-swap](https://github.com/raydium-io/raydium-cp-swap) at `bdeee23`, under [Apache-2.0](LICENSES/Apache-2.0.txt). The fixture includes its upgradeable-loader header, as described in the [fixture guide](tests/fixtures/README.md). Its upstream source has no NOTICE file.

## Public chain data

The retained `tests/fixtures/accounts` files, Jupiter account replies, route captures and reference-rent receipt are public Solana state or recorded provider/RPC responses used for deterministic tests. The short.win authors claim no rights in this data; rights remain with their respective owners. See [the public-chain-data notice](LICENSES/LicenseRef-Public-Chain-Data.txt).

The pure Jupiter parser and pooled accounting crates are first-party source. No direct DEX pricing implementation or downloaded SDK source is vendored into those crates.
