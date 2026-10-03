# @shortwin/math-wasm

The browser build of [`crates/shortwin-math-wasm`](../../crates/shortwin-math-wasm) (the
wasm-bindgen facade over `shortwin-math`; ARCHITECTURE §1.2, §7.3). The web app
(`apps/web`) computes every preview with it — quotes, the D4 margin, CR, the liquidation
price, rent, waterfalls, caps, tiers and the lender pool — so the UI shows exactly what the
program will compute. The exports and JS conventions are documented in the crate's README.

`pkg/` is **generated, not committed** (the `.wasm` embeds absolute registry paths and is not
byte-reproducible across machines):

```sh
pnpm --filter @shortwin/math-wasm build        # skipped when pkg/ matches the crate sources
pnpm --filter @shortwin/math-wasm build:force  # always rebuild
```

The script runs `cargo build -p shortwin-math-wasm --target wasm32-unknown-unknown --release
--locked` (honouring `CARGO_TARGET_DIR`) and `wasm-bindgen --target web`. It needs the
wasm-bindgen CLI at the crate's exact pin, **0.2.129**, from `$WASM_BINDGEN`, `PATH` or
`~/.cargo/bin`:

```sh
cargo install wasm-bindgen-cli --version =0.2.129 --locked
```

`pkg/.inputs-sha256` records a SHA-256 of the three crates' sources and manifests
(`shortwin-math`, `shortwin-venue-state`, `shortwin-math-wasm`) and the root `Cargo.lock`; a
matching stamp skips the build, so `pnpm --filter web build` needs neither cargo nor
wasm-bindgen once `pkg/` is current.

`--target web` output: the app calls the default export `init()` once (Vite serves the
`.wasm` as an asset through `new URL(…, import.meta.url)`); Node tests call
`initSync({ module: <bytes of pkg/shortwin_math_wasm_bg.wasm> })` instead
(`apps/web/src/math/load.ts`).
