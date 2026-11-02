import "./style.css";
import { installLocalDemoWallet, isLoopbackUrl } from "./local-demo-wallet.ts";
import {
  address,
  createNoopSigner,
  createSolanaRpc,
  getBase58Encoder,
  type Address,
  type Instruction,
  type AddressesByLookupTableAddress,
  type Signature,
} from "@solana/kit";
import * as sw from "@shortwin/simple-client";
import {
  claimable,
  depositShares,
  formatAmount,
  rawAmount,
  redemption,
} from "./amounts.ts";
import { boundBuyInput, managedCall } from "./managed.ts";
import { IntentRevision, reconcileSignatureStatus } from "./intent.ts";
import {
  assertConnection,
  connectWallet,
  discoverWallets,
  signTransaction,
  watchWallet,
  wallets,
  type Connection,
} from "./wallet.ts";
const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `<main>
 <header class="top">
  <a class="brand" href="#" aria-label="short.win home"><svg class="brand-mark" width="32" height="32" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z"/><path class="brand-mark-win" d="M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z"/></svg>short<span>.win</span></a>
  <nav class="top-nav" aria-label="Main navigation"><a class="selected" href="#workspace">Workspace</a><a href="#how-it-works">How it works <span aria-hidden="true">↗</span></a></nav>
  <div class="wallet"><small id="local-demo-note" hidden>Local test wallet · no real funds</small><span class="network-badge"><span aria-hidden="true"></span><span id="environment-name">Local fork</span></span><select id="wallet" aria-label="Choose a wallet"></select><button id="connect">Connect wallet <span aria-hidden="true">↗</span></button></div>
 </header>
 <section class="hero" aria-labelledby="hero-title">
  <div class="hero-copy"><div class="eyebrow"><span class="eyebrow-line" aria-hidden="true"></span>A different side of the market</div><h1 id="hero-title">Put your tokens<br>to <em>work.</em></h1><p>Lend tokens to share in paid SOL fees.<br>Borrow and short with clear terms and SOL collateral.</p><div class="hero-tags"><span><span aria-hidden="true">↗</span> Pooled liquidity</span><span><span aria-hidden="true">◷</span> Fixed borrowing terms</span><span><span aria-hidden="true">◇</span> Your wallet, your approval</span></div></div>
  <div class="hero-visual" aria-hidden="true"><div class="orbit orbit-one"></div><div class="orbit orbit-two"></div><div class="orbit-axis"></div><div class="visual-token token-accent"><span>↗</span></div><div class="visual-token token-dark"><span>↘</span></div><span class="orbit-label label-lend">LEND & EARN</span><span class="orbit-label label-short">BORROW & SHORT</span><span class="visual-caption">TWO SIDES. ONE POOL.</span></div>
 </section>
 <div class="notice"><span class="notice-icon" aria-hidden="true">i</span><p><strong id="network-summary">Mainnet fork. Test funds.</strong> <span id="network-description">This demo uses a local fork, test assets, and synthetic price history. Review every transaction in your wallet.</span></p><a href="#protocol-details">Understand the risks <span aria-hidden="true">↗</span></a></div>
 <section class="card market-card" id="workspace" aria-labelledby="market-title">
  <div class="section-heading"><div><div class="section-kicker">01 / SELECT A MARKET</div><h2 id="market-title">Start with a token.</h2></div><span class="subtle-tag">Solana SPL & Token-2022</span></div>
  <div class="market-search"><div class="mint-field"><label for="mint">Token mint address</label><div class="input-with-icon"><span aria-hidden="true">⌕</span><input id="mint" placeholder="Paste a Solana token mint address" spellcheck="false" autocomplete="off"></div></div><button id="load">Load pool <span aria-hidden="true">→</span></button></div>
  <p class="muted eligibility" id="eligibility">Choose an eligible token to view its pool. Supported mints have no freeze authority; token behavior extensions such as transfer fees and hooks are excluded.</p>
  <details class="advanced market-advanced"><summary>Pool registration & generation <span aria-hidden="true">+</span></summary><div class="fields"><div><label for="generation">Pool generation</label><input id="generation" placeholder="Current pool (leave blank)" inputmode="numeric"></div><div class="registration-action"><p class="muted">New token? Load its mint to check eligibility, then prepare a pool registration.</p><button class="secondary" id="register">Prepare registration <span aria-hidden="true">↗</span></button></div></div><p class="muted">Registration alone does not enable shorting. A token also needs qualifying price history and a complete trade route.</p></details>
 </section>
 <div class="grid"><section class="card trading-card" aria-labelledby="trading-title">
  <div class="section-heading"><div><div class="section-kicker">02 / MAKE YOUR MOVE</div><h2 id="trading-title">Your pool workspace</h2></div><span class="pool-tag"><span aria-hidden="true"></span>On-chain balances</span></div>
  <div class="stats"><div class="stat"><small>Available in pool</small><strong id="cash">—</strong><span class="stat-unit">POOL TOKENS</span></div><div class="stat"><small>Currently borrowed</small><strong id="debt">—</strong><span class="stat-unit">POOL TOKENS</span></div><div class="stat"><small>Paid fee reserve</small><strong id="income">—</strong><span class="stat-unit">SOL</span></div></div>
  <nav class="tabs" aria-label="Pool actions" role="tablist"><button class="active" data-tab="lend" role="tab" aria-selected="true" aria-controls="lend" id="tab-lend"><span aria-hidden="true">↗</span> Lend</button><button data-tab="short" role="tab" aria-selected="false" aria-controls="short" id="tab-short" tabindex="-1"><span aria-hidden="true">↘</span> Short</button><button data-tab="positions" role="tab" aria-selected="false" aria-controls="positions" id="tab-positions" tabindex="-1"><span aria-hidden="true">▤</span> Positions</button></nav>
  <section class="panel" id="lend" role="tabpanel" aria-labelledby="tab-lend"><div class="panel-intro"><h3>Supply tokens. Share in the fees.</h3><p class="muted">Deposit into the pool to receive shares and earn a portion of SOL fees paid by borrowers.</p></div><label for="token-account">Your token account</label><input id="token-account" placeholder="Filled when you connect a wallet and load a pool" spellcheck="false" autocomplete="off"><div class="fields"><div><label for="deposit-amount">Tokens to lend</label><div class="amount-field"><input id="deposit-amount" inputmode="decimal" placeholder="0.00"><span>TOKENS</span></div></div><div><label for="withdraw-shares">Shares to redeem</label><div class="amount-field"><input id="withdraw-shares" inputmode="decimal" placeholder="0.00"><span>SHARES</span></div></div></div><div class="actions"><button id="deposit">Prepare deposit <span aria-hidden="true">↗</span></button><button class="secondary" id="withdraw">Withdraw</button><button class="text-button" id="claim">Claim SOL fees <span aria-hidden="true">→</span></button></div><div class="receipt-box"><span class="receipt-icon" aria-hidden="true">◇</span><p class="muted" id="receipt">Connect your wallet and load a pool to see your shares and claimable SOL.</p></div><details class="advanced"><summary>How withdrawals & fee claims work <span aria-hidden="true">+</span></summary><p class="muted">Withdrawals depend on available tokens. The final shares cannot exit while a loan remains. Share changes settle paid SOL first; fractions stay while shares remain. A full receipt exit clears its sub-lamport fraction. The final holder with no debt receives tracked SOL dust.</p></details></section>
  <section class="panel" id="short" hidden role="tabpanel" aria-labelledby="tab-short"><div class="panel-intro"><h3>Take the other side.</h3><p class="muted">Borrow tokens, sell through Jupiter, and repay the full quantity to close. Set your spending limits below.</p></div><div class="fields"><div><label for="quantity">Tokens to short</label><div class="amount-field"><input id="quantity" inputmode="decimal" placeholder="0.00"><span>TOKENS</span></div></div><div><label for="min-sale">Minimum sale proceeds</label><div class="amount-field"><input id="min-sale" inputmode="decimal" placeholder="0.00"><span>SOL</span></div></div><div><label for="max-margin">Maximum collateral contribution</label><div class="amount-field"><input id="max-margin" inputmode="decimal" placeholder="0.00"><span>SOL</span></div></div><div><label for="max-fee">Maximum upfront term fee</label><div class="amount-field"><input id="max-fee" inputmode="decimal" placeholder="0.00"><span>SOL</span></div></div></div><div class="receipt-box"><span class="receipt-icon" aria-hidden="true">◷</span><p class="muted" id="terms">Load a pool to see its fixed term, fee, and collateral requirements.</p></div><div class="actions"><button id="open">Prepare short <span aria-hidden="true">↘</span></button></div><details class="advanced"><summary>Price history & route information <span aria-hidden="true">+</span></summary><p class="muted">New shorts require fresh price history for the exact quantity and an executable route. A warm-up request may take time to qualify.</p><div class="actions"><button class="secondary" id="warm-reference">Request history warm-up</button><button class="secondary" id="route-request">Show route addresses</button></div><pre class="status" id="route-hint"></pre></details></section>
  <section class="panel" id="positions" hidden role="tabpanel" aria-labelledby="tab-positions"><div class="panel-intro"><h3>Keep your positions in view.</h3><p class="muted">Add collateral, repay tokens, or prepare a full buyback through Jupiter.</p></div><label for="loans">Your positions in this market</label><select id="loans"><option value="">Connect a wallet and load a pool</option></select><pre class="loan-detail" id="loan-detail"></pre><label for="margin-amount">Additional SOL collateral</label><div class="amount-field"><input id="margin-amount" inputmode="decimal" placeholder="0.00"><span>SOL</span></div><div class="actions"><button id="margin">Add collateral</button><button class="secondary" id="repay">Repay in tokens</button></div><hr class="separator"><h3>Close with a buyback</h3><div class="fields"><div><label for="buy-input">Jupiter buyback input</label><div class="amount-field"><input id="buy-input" inputmode="decimal" placeholder="0.00"><span>SOL</span></div></div><div><label for="max-buyback">Maximum buyback cost</label><div class="amount-field"><input id="max-buyback" inputmode="decimal" placeholder="0.00"><span>SOL</span></div></div></div><div class="actions"><button id="close">Prepare full close <span aria-hidden="true">↗</span></button></div><p class="muted">A close must repay every borrowed token. Partial fills do not close a position.</p><details class="advanced"><summary>Closing route information <span aria-hidden="true">+</span></summary><button class="secondary" id="close-route-request">Show closing route addresses</button><pre class="status" id="close-hint"></pre></details></section>
 </section><aside>
  <section class="card review" id="review" hidden aria-labelledby="review-title"><div class="section-kicker">03 / REVIEW & SIGN</div><h2 id="review-title">You're in control.</h2><p id="review-text"></p><p class="muted">Simulation reflects current RPC state. Liquidity, limits, fees, or blockhash validity may change before signing. Only the reviewed message will be submitted.</p><button id="sign" class="full">Review in wallet & send <span aria-hidden="true">↗</span></button><button id="cancel" class="secondary full">Cancel</button></section>
  <section class="card activity-card" aria-labelledby="activity-title"><div class="aside-heading"><h2 id="activity-title">Activity</h2><span class="activity-indicator" aria-hidden="true"></span></div><div class="activity-content"><span class="activity-symbol" aria-hidden="true">↳</span><pre class="status" id="status" role="status" aria-live="polite">Your workspace is ready.\nLoad a token pool to get started.</pre></div><div class="activity-footer"><span aria-hidden="true">◇</span> Every transaction needs your approval</div></section>
  <section class="card guide-card" id="how-it-works" aria-labelledby="guide-title"><div class="section-kicker">A LITTLE CONTEXT</div><h2 id="guide-title">Two ways to participate.</h2><div class="guide-item"><span class="guide-icon" aria-hidden="true">↗</span><div><h3>Lend your tokens</h3><p class="muted">Supply liquidity and share in SOL fees actually paid to the pool.</p></div></div><div class="guide-item"><span class="guide-icon" aria-hidden="true">↘</span><div><h3>Open a short</h3><p class="muted">Borrow and sell tokens with SOL collateral and a fixed upfront fee.</p></div></div><div class="guide-note"><span aria-hidden="true">◷</span><p>Closing early does not refund the term fee. Lending does not guarantee immediate withdrawal.</p></div><details class="advanced" id="protocol-details"><summary>Understand the risks <span aria-hidden="true">+</span></summary><p class="muted">The protocol is experimental; release qualification, an external audit, and a bug bounty remain pending. Price and completed-history reports come from the protocol's service. Trusted reports do not guarantee a liquid route or protect against token price jumps.</p><p class="muted">If full repayment remains impossible after the deadline and its 24-hour grace period, remaining collateral compensates lenders after the capped keeper fee. The borrower receives no economic payout after grace, even if a recent price looks healthy. Account rent is separate.</p><p class="muted">Borrowing caps apply per market. Deposits and the number of markets are uncapped. Unknown or warming price history prevents new shorts.</p></details></section>
 </aside></div>
 <details class="settings card"><summary><span><span class="settings-symbol" aria-hidden="true">⚙</span> Advanced settings</span><span class="settings-hint">Connection, routing & spending limits <span aria-hidden="true">+</span></span></summary><div class="settings-content"><div class="settings-section"><h3>Connection</h3><div class="fields"><div><label for="rpc">Public Solana RPC URL</label><input id="rpc" type="url" spellcheck="false" autocomplete="off"></div><div><label for="chain">Wallet signing chain</label><select id="chain"><option value="solana:devnet">Devnet / local fork</option><option value="solana:mainnet">Mainnet</option><option value="solana:testnet">Testnet</option></select></div></div><label for="service-url">Public quote & history service URL</label><input id="service-url" type="url" placeholder="https://your-public-service.example" spellcheck="false" autocomplete="off"><p class="muted">RPC and service URLs are public browser settings. Never enter private credentials. The loaded configuration's genesis hash must match the RPC before a transaction can be prepared. Local validators use the wallet's Devnet signing chain.</p><div class="program-address muted" id="program"></div></div><div class="settings-section"><h3>Route & spending limits</h3><label for="route-bundle">Jupiter V2 build response JSON (optional)</label><textarea id="route-bundle" spellcheck="false" placeholder="Paste a real ExactIn V2 build response for the displayed route authority"></textarea><p class="muted">The quote service normally supplies routes. This override binds the mints, whole-position quantity, and route accounts. Setup, cleanup, and tip instructions are excluded.</p><div class="fields"><div><label for="rent-cap">Maximum new account rent (SOL)</label><input id="rent-cap" value="0.1" inputmode="decimal"></div><div><label for="creation-fee-cap">Maximum market creation fee (SOL)</label><input id="creation-fee-cap" value="0.1" inputmode="decimal"></div><div><label for="temporary-cap">Maximum total swap advance (SOL)</label><input id="temporary-cap" value="0.01" inputmode="decimal"></div><div><label for="setup-cap">Maximum native DEX setup (SOL)</label><input id="setup-cap" value="0.005" inputmode="decimal"></div></div><p class="muted">DEX setup is paid separately from collateral and the term fee. Unused funds return to the payer; spent setup is nonrefundable. The swap advance must cover setup and temporary token-account deposits.</p></div></div></details>
 <footer><a class="brand footer-brand" href="#" aria-label="short.win home"><svg class="brand-mark" width="32" height="32" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z"/><path class="brand-mark-win" d="M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z"/></svg>short<span>.win</span></a><span>Choose your side. Know your terms.</span><span class="footer-network">Built on Solana <span aria-hidden="true">·</span> <span id="footer-environment">Local fork</span></span><span class="footer-program" id="footer-program"></span></footer>
</main>`;
const el = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const input = (id: string) => el<HTMLInputElement>(id).value.trim();
const text = (id: string, value: string) => {
  el(id).textContent = value;
};
el<HTMLInputElement>("rpc").value = import.meta.env.VITE_RPC_URL?.trim() || "http://127.0.0.1:8899";
el<HTMLInputElement>("service-url").value = import.meta.env.VITE_SERVICE_URL?.trim() || "http://127.0.0.1:8787";
const cluster = import.meta.env.VITE_CLUSTER?.trim() || "localnet";
const environmentName = cluster === "localnet" ? "Local fork" : cluster === "devnet" ? "Devnet" : "Custom network";
text("environment-name", environmentName);
text("footer-environment", environmentName);
if (cluster !== "localnet") {
  text("network-summary", cluster === "devnet" ? "A place to explore. On Devnet." : "Check your network before signing.");
  text("network-description", "This is an experimental protocol. Review the configured network, protocol risks, and every transaction in your wallet.");
}
text("program", "Program: " + sw.PROGRAM);
text("footer-program", sw.PROGRAM);
let connection: Connection | undefined;
type Context = {
  mint: Address;
  mintInfo: {
    tokenProgram: Address;
    decimals: number;
  };
  config: sw.Config;
  policy: sw.Policy;
  market: sw.Market | null;
  pool: sw.PoolGeneration | null;
  receipt: sw.LenderReceipt | null;
  marketKey: Address;
  poolKey: Address | null;
};
let context: Context | undefined;
let ownedLoans: {
  address: Address;
  data: sw.Loan;
}[] = [];
let pending:
  | {
      prepared: sw.PreparedTransaction;
      url: string;
      connection: Connection;
      chain: string;
    }
  | undefined;
let unresolved:
  | {
      signature: Signature;
      url: string;
      lastValidBlockHeight: bigint;
      broadcastStarted: boolean;
    }
  | undefined;
let busy = false;
const intent = new IntentRevision();
let activeRevision = intent.capture();
let stopWalletWatch: () => void = () => {};
function invalidate() {
  intent.invalidate();
  pending = undefined;
  el("review").hidden = true;
}
function status(message: string) {
  el("status").dataset.state = "info";
  text("status", message);
}
function error(e: unknown) {
  status(e instanceof Error ? e.message : String(e));
  el("status").dataset.state = "error";
}
async function task(fn: () => Promise<void>, keepReview = false) {
  if (busy) return;
  busy = true;
  app.dataset.busy = "true";
  app.setAttribute("aria-busy", "true");
  if (!keepReview) invalidate();
  activeRevision = intent.capture();
  try {
    await fn();
  } catch (e) {
    error(e);
  } finally {
    el<HTMLButtonElement>("sign").disabled = false;
    if (!pending) el("review").hidden = true;
    busy = false;
    app.dataset.busy = "false";
    app.setAttribute("aria-busy", "false");
  }
}
function signer() {
  if (!connection) throw new Error("Connect a wallet first");
  return createNoopSigner(connection.address);
}
function ctx() {
  if (!context) throw new Error("Load an eligible mint first");
  return context;
}
function livePool() {
  const c = ctx();
  if (!c.market || !c.pool || !c.poolKey)
    throw new Error("This token has no registered pool");
  return { ...c, market: c.market, pool: c.pool, poolKey: c.poolKey };
}
async function verifyGenesis(c: Context, url: string) {
  const genesis = await createSolanaRpc(url).getGenesisHash().send();
  if (
    (genesis === "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d") !==
    (input("chain") === "solana:mainnet")
  )
    throw new Error("Mainnet RPC and wallet signing chain must match");
  const bytes = getBase58Encoder().encode(genesis);
  if (
    bytes.length !== 32 ||
    bytes.some((x, i) => x !== c.config.genesisHash[i])
  )
    throw new Error(
      "RPC genesis does not match the loaded program configuration",
    );
}
async function review(
  label: string,
  instructions: readonly Instruction[],
  tables: AddressesByLookupTableAddress = {},
) {
  const c = ctx();
  intent.assert(activeRevision);
  if (!connection) throw new Error("Connect a wallet first");
  const url = input("rpc");
  if (unresolved) {
    if (unresolved.url !== url)
      throw new Error(
        `Signature ${unresolved.signature} has an unknown outcome on the previous RPC. Reconnect to that RPC and reconcile it before signing another transaction.`,
      );
    const rpc = createSolanaRpc(url);
    const {
      value: [known],
    } = await rpc
      .getSignatureStatuses([unresolved.signature], {
        searchTransactionHistory: true,
      })
      .send();
    const reconciliation = reconcileSignatureStatus(
      known,
      known === null &&
        (await rpc.getBlockHeight({ commitment: "confirmed" }).send()) >
          unresolved.lastValidBlockHeight,
    );
    if (reconciliation === "failed") unresolved = undefined;
    else if (reconciliation === "confirmed") {
      const signature = unresolved.signature;
      unresolved = undefined;
      throw new Error(
        `The previous transaction confirmed as ${signature}. Refresh the pool before preparing another transaction.`,
      );
    } else if (reconciliation === "expired-absent")
      unresolved = undefined;
    else if (known)
      throw new Error(
        `Signature ${unresolved.signature} was observed as ${known.confirmationStatus ?? "processed"} and may still confirm. Wait for confirmation or an explicit failure before signing another transaction.`,
      );
    else
      throw new Error(
        `Signature ${unresolved.signature} may still land. Wait for confirmation or blockhash expiry before signing another transaction.`,
      );
  }
  await verifyGenesis(c, url);
  status("Simulating transaction…");
  const prepared = await sw.prepareTransaction(
    url,
    connection.address,
    instructions,
    tables,
  );
  intent.assert(activeRevision);
  pending = { prepared, url, connection, chain: input("chain") };
  text(
    "review-text",
    `${label}

Mint: ${c.mint}
Network RPC: ${url}
Network: ${cluster === "localnet" && isLoopbackUrl(url) && input("chain") === "solana:devnet" ? "Local fork · test funds" : input("chain")}
Wallet: ${connection.address}
Estimated network fee: ${formatAmount(prepared.feeLamports, 9)} SOL
Simulation: ${prepared.units} compute units
Wire size: ${prepared.bytes.length} bytes`,
  );
  el("review").hidden = false;
  el("review").scrollIntoView({ block: "start", behavior: "auto" });
  status(
    "Simulation passed. Review the terms and approve the transaction in your wallet only if they match your intent.",
  );
}
async function load() {
  invalidate();
  const loadedRevision = intent.capture();
  context = undefined;
  ownedLoans = [];
  const url = input("rpc");
  const mint = address(input("mint"));
  status("Reading mint and pool accounts…");
  const mintInfo = await sw.readMint(url, mint);
  const config = await sw.readAccount(
    url,
    await sw.configAddress(),
    sw.getConfigDecoder(),
    sw.ShortwinSimpleAccount.Config,
  );
  if (!config)
    throw new Error("This new program is not initialized on the selected RPC");
  const policy = await sw.readAccount(
    url,
    config.currentPolicy,
    sw.getPolicyDecoder(),
    sw.ShortwinSimpleAccount.Policy,
  );
  if (!policy) throw new Error("Current policy is missing");
  const marketKey = await sw.marketAddress(mint);
  const market = await sw.readAccount(
    url,
    marketKey,
    sw.getMarketDecoder(),
    sw.ShortwinSimpleAccount.Market,
  );
  let poolKey: Address | null = null,
    pool: sw.PoolGeneration | null = null,
    receipt: sw.LenderReceipt | null = null;
  if (market) {
    const generation =
      input("generation") === ""
        ? market.activeGeneration
        : rawAmount(input("generation"), 0);
    poolKey = await sw.poolAddress(marketKey, generation);
    pool = await sw.readAccount(
      url,
      poolKey,
      sw.getPoolGenerationDecoder(),
      sw.ShortwinSimpleAccount.PoolGeneration,
    );
    if (!pool) throw new Error("Selected pool generation does not exist");
    if (connection)
      receipt = await sw.readAccount(
        url,
        await sw.receiptAddress(poolKey, connection.address),
        sw.getLenderReceiptDecoder(),
        sw.ShortwinSimpleAccount.LenderReceipt,
      );
  }
  intent.assert(loadedRevision);
  context = {
    mint,
    mintInfo,
    config,
    policy,
    market,
    marketKey,
    pool,
    poolKey,
    receipt,
  };
  const decimals = mintInfo.decimals;
  text("cash", pool ? formatAmount(pool.cash, decimals) : "Unregistered");
  text("debt", pool ? formatAmount(pool.principalOut, decimals) : "—");
  text("income", pool ? formatAmount(pool.solReserve, 9) : "—");
  text(
    "eligibility",
    market
      ? `Registered ${mintInfo.tokenProgram === sw.TOKEN ? "SPL" : "Token-2022"} mint · ${decimals} decimals · generation ${pool!.generation}${pool!.retired ? " (retired; old claims remain available)" : ""}. New shorts also require fresh finite quantity-specific pricing and completed history.`
      : "Eligible mint. Public registration creates a token pool; borrowing remains unavailable until the in-house service provides qualifying completed history and a route.",
  );
  if (connection) {
    const tokenAccount = await sw.associatedToken(
      connection.address,
      mint,
      mintInfo.tokenProgram,
    );
    intent.assert(loadedRevision);
    el<HTMLInputElement>("token-account").value = tokenAccount;
    text(
      "receipt",
      receipt
        ? `Your shares: ${formatAmount(receipt.shares, decimals)} · Whole SOL claim: ${formatAmount(claimable(pool!, receipt), 9)} SOL`
        : "No lender receipt in this generation.",
    );
  }
  const t = policy.terms;
  text(
    "terms",
    `Term: ${t.termSeconds / 3600} hours. Upfront fee: ${t.termFeeBps / 100}% of conservative opening notional. No early-close refund. Opening collateral ratio: ${t.crOpenBps / 100}%. Grace: ${t.defaultGraceSeconds / 3600} hours. ${config.opensPaused ? "New shorts are paused." : ""}`,
  );
  const select = el<HTMLSelectElement>("loans");
  select.replaceChildren(new Option("Select a position", ""));
  if (connection && market) {
    const rows = await sw.listAccounts(url);
    intent.assert(loadedRevision);
    ownedLoans = rows
      .filter((r) => r.kind === sw.ShortwinSimpleAccount.Loan)
      .map((r) => ({ address: r.address, data: r.data as sw.Loan }))
      .filter(
        (r) =>
          r.data.borrower === connection!.address &&
          r.data.market === marketKey,
      );
    for (const loan of ownedLoans)
      select.add(
        new Option(
          `${loan.address.slice(0, 8)}… · ${formatAmount(loan.data.q, decimals)} tokens`,
          loan.address,
        ),
      );
  }
  text("loan-detail", "");
  status(
    "Accounts refreshed. Transaction amounts and signed minimums will be reviewed before wallet approval.",
  );
}
function selectedLoan() {
  const loan = ownedLoans.find((l) => l.address === input("loans"));
  if (!loan) throw new Error("Select one of your positions");
  return loan;
}
async function loanContext() {
  const c = livePool();
  const loan = selectedLoan();
  const pool = await sw.readAccount(
    input("rpc"),
    loan.data.pool,
    sw.getPoolGenerationDecoder(),
    sw.ShortwinSimpleAccount.PoolGeneration,
  );
  if (!pool) throw new Error("Position pool is missing");
  return { c, loan, pool };
}
async function routeBuild(
  binding: sw.RouteBinding,
  loan: Address,
  borrower: Address,
  nonce?: bigint,
): Promise<sw.BuildResponse> {
  const raw = input("route-bundle");
  if (raw) return JSON.parse(raw) as sw.BuildResponse;
  const {
    mint,
    tokenProgram,
    side,
    quantity,
    inputAmount,
    minimumOutput,
    executionAuthority,
    executionBase,
    executionWsol,
    setupLamports,
  } = binding;
  const result = await managedCall(input("service-url"), "/v1/route", {
    mint,
    tokenProgram,
    side,
    quantity,
    inputAmount,
    minimumOutput,
    executionAuthority,
    executionBase,
    executionWsol,
    setupLamports: setupLamports ?? 0n,
    loan,
    borrower,
    ...(nonce === undefined ? {} : { nonce }),
  });
  if (!result.build)
    throw new Error("The in-house service has no executable route");
  return result.build as sw.BuildResponse;
}
async function common(c: ReturnType<typeof livePool>, owner: Address) {
  return {
    owner: signer(),
    market: c.marketKey,
    pool: c.poolKey,
    receipt: await sw.receiptAddress(c.poolKey, owner),
    mint: c.mint,
    ownerTokens: address(input("token-account")),
    tokenVault: await sw.vaultAddress(c.poolKey),
    tokenProgram: c.mintInfo.tokenProgram,
    eventAuthority: await sw.eventAuthority(),
    program: sw.PROGRAM,
  };
}
el("load").onclick = () => void task(load);
el("register").onclick = () =>
  void task(async () => {
    const c = ctx();
    const owner = signer();
    if (c.market) throw new Error("Market already registered");
    const market = await sw.marketAddress(c.mint),
      pool = await sw.poolAddress(market, 0n);
    const maxCreationFee = rawAmount(input("creation-fee-cap"), 9);
    const maxSetupRent = rawAmount(input("rent-cap"), 9);
    const instruction = await sw.getRegisterMarketInstructionAsync({
      creator: owner,
      config: await sw.configAddress(),
      policy: c.config.currentPolicy,
      market,
      pool,
      mint: c.mint,
      tokenVault: await sw.vaultAddress(pool),
      feeRecipient: c.config.feeRecipient,
      tokenProgram: c.mintInfo.tokenProgram,
      eventAuthority: await sw.eventAuthority(),
      program: sw.PROGRAM,
      maxCreationFee,
      maxSetupRent,
    });
    await review(
      `Register this eligible token. Maximum creation fee: ${formatAmount(maxCreationFee, 9)} SOL. Maximum new account rent: ${formatAmount(maxSetupRent, 9)} SOL. Registration does not enable borrowing without qualifying price history.`,
      [instruction],
    );
  });
el("deposit").onclick = () =>
  void task(async () => {
    const c = livePool(),
      owner = signer();
    if (c.pool.retired || c.pool.generation !== c.market.activeGeneration)
      throw new Error("New deposits use the current active pool");
    const amount = rawAmount(input("deposit-amount"), c.mintInfo.decimals);
    if (amount === 0n) throw new Error("Enter a positive token amount");
    const minShares = depositShares(amount, c.pool);
    if (minShares === 0n)
      throw new Error("Deposit is too small to mint a share");
    const instruction = await sw.getDepositInstructionAsync({
      ...(await common(c, owner.address)),
      amount,
      minShares,
      maxReceiptRent: rawAmount(input("rent-cap"), 9),
    });
    await review(
      `Lend ${formatAmount(amount, c.mintInfo.decimals)} tokens. Signed minimum shares: ${formatAmount(minShares, c.mintInfo.decimals)}. Only future actually paid SOL income belongs to these new shares.`,
      [instruction],
    );
  });
el("withdraw").onclick = () =>
  void task(async () => {
    const c = livePool(),
      owner = signer();
    if (!c.receipt) throw new Error("No receipt in this generation");
    const shares = rawAmount(
      input("withdraw-shares"),
      c.mintInfo.decimals,
      2n ** 128n - 1n,
    );
    if (shares === 0n || shares > c.receipt.shares)
      throw new Error("Enter shares within your balance");
    const minTokens = redemption(shares, c.pool);
    if (minTokens > c.pool.cash)
      throw new Error(
        "Insufficient available token cash; choose fewer shares or wait for repayment",
      );
    const minSol =
      shares === c.pool.shares
        ? c.pool.solReserve
        : claimable(c.pool, c.receipt);
    const instruction = await sw.getWithdrawInstructionAsync({
      ...(await common(c, owner.address)),
      shares,
      minTokens,
      minSol,
    });
    await review(
      `Redeem ${formatAmount(shares, c.mintInfo.decimals)} shares. Signed minimum: ${formatAmount(minTokens, c.mintInfo.decimals)} tokens and ${formatAmount(minSol, 9)} SOL. A full receipt exit clears its sub-lamport fraction. Receipt rent is returned when fully closed.`,
      [instruction],
    );
  });
el("claim").onclick = () =>
  void task(async () => {
    const c = livePool(),
      owner = signer();
    if (!c.receipt) throw new Error("No receipt in this generation");
    const minSol = claimable(c.pool, c.receipt);
    if (minSol === 0n)
      throw new Error(
        "No whole lamport is claimable yet; fractions remain with your shares",
      );
    const instruction = await sw.getClaimSolInstructionAsync({
      owner,
      pool: c.poolKey,
      receipt: await sw.receiptAddress(c.poolKey, owner.address),
      eventAuthority: await sw.eventAuthority(),
      program: sw.PROGRAM,
      minSol,
    });
    await review(
      `Claim at least ${formatAmount(minSol, 9)} SOL of actually paid income. Your shares remain in the pool.`,
      [instruction],
    );
  });
el("route-request").onclick = () =>
  void task(async () => {
    const c = livePool(),
      owner = signer();
    const q = rawAmount(input("quantity"), c.mintInfo.decimals);
    const loan = await sw.loanAddress(
      c.poolKey,
      owner.address,
      c.market.nextLoanNonce,
    );
    const execution = await sw.executionAddresses(loan);
    text(
      "route-hint",
      `Sell ${q} raw units of ${c.mint} into WSOL.
Taker: ${execution.executionAuthority}
Destination token account: ${execution.executionWsol}
Whole-position nonce: ${c.market.nextLoanNonce}
ExactIn; wrapAndUnwrapSol=false. Paste the real build response in local development settings.`,
    );
  });
el("warm-reference").onclick = () =>
  void task(async () => {
    const c = livePool();
    const quantity = rawAmount(input("quantity"), c.mintInfo.decimals);
    if (quantity <= 0n) throw new Error("Enter a positive quantity first");
    status("Requesting exact-quantity history warm-up…");
    const result = await managedCall(input("service-url"), "/v1/requests", {
      mint: c.mint,
      quantity,
    });
    text("route-hint", JSON.stringify(result, null, 2));
    status(
      "History request accepted. The service must observe completed epochs and publish a fresh reference; retry Prepare short after warm-up.",
    );
  });
el("open").onclick = () =>
  void task(async () => {
    const c = livePool(),
      owner = signer();
    if (c.config.opensPaused || c.config.pricesPaused)
      throw new Error("New shorts or prices are paused");
    if (c.pool.retired || c.pool.generation !== c.market.activeGeneration)
      throw new Error("New shorts use the active pool");
    const q = rawAmount(input("quantity"), c.mintInfo.decimals);
    if (q === 0n) throw new Error("Enter a positive quantity");
    const reference = await sw.referenceAddress(
      c.marketKey,
      c.config.currentPolicy,
      q,
    );
    const quote = await sw.readAccount(
      input("rpc"),
      reference,
      sw.getPriceReferenceDecoder(),
      sw.ShortwinSimpleAccount.PriceReference,
    );
    if (!quote || !quote.finite)
      throw new Error(
        "Price/history is Unknown or still warming for this exact quantity",
      );
    const loan = await sw.loanAddress(
      c.poolKey,
      owner.address,
      c.market.nextLoanNonce,
    );
    const execution = await sw.executionAddresses(loan);
    const minProceeds = rawAmount(input("min-sale"), 9);
    const maxMargin = rawAmount(input("max-margin"), 9);
    const maxTermFee = rawAmount(input("max-fee"), 9);
    const binding: sw.RouteBinding = {
      mint: c.mint,
      tokenProgram: c.mintInfo.tokenProgram,
      side: "sell",
      quantity: q,
      inputAmount: q,
      minimumOutput: minProceeds,
      setupLamports: rawAmount(input("setup-cap"), 9),
      ...execution,
    };
    const route = await sw.planBundle(
      await routeBuild(binding, loan, owner.address, c.market.nextLoanNonce),
      binding,
    );
    const instruction = await sw.getOpenPositionInstructionAsync({
      borrower: owner,
      config: await sw.configAddress(),
      policy: c.config.currentPolicy,
      market: c.marketKey,
      pool: c.poolKey,
      loan,
      exposure: await sw.exposureAddress(c.poolKey, owner.address),
      reference,
      mint: c.mint,
      tokenVault: await sw.vaultAddress(c.poolKey),
      tokenProgram: c.mintInfo.tokenProgram,
      feeRecipient: c.config.feeRecipient,
      ...execution,
      eventAuthority: await sw.eventAuthority(),
      program: sw.PROGRAM,
      nonce: c.market.nextLoanNonce,
      q,
      minProceeds,
      maxMargin,
      maxTermFee,
      temporaryFunding: rawAmount(input("temporary-cap"), 9),
      maxAccountRent: rawAmount(input("rent-cap"), 9),
      minimumReferenceSequence: quote.sequence,
      route: route.route,
    });
    const tables = await sw.readLookupTables(
      input("rpc"),
      route.lookupTableKeys,
    );
    await review(
      `Short ${formatAmount(q, c.mintInfo.decimals)} tokens. Minimum sale: ${formatAmount(minProceeds, 9)} SOL. Maximum collateral contribution: ${formatAmount(maxMargin, 9)} SOL. Maximum fixed upfront fee: ${formatAmount(maxTermFee, 9)} SOL. Separately paid native DEX setup allowance: up to ${formatAmount(binding.setupLamports??0n, 9)} SOL; spent setup is not refunded. Fee is not refunded on early close. Deadline + 24h grace can forfeit all borrower economic collateral.`,
      [sw.appendRoute(instruction, route)],
      tables,
    );
  });
el<HTMLSelectElement>("loans").onchange = () => {
  invalidate();
  try {
    const l = selectedLoan().data;
    text(
      "loan-detail",
      `Loan: ${input("loans")}
Borrowed: ${formatAmount(l.q, ctx().mintInfo.decimals)} tokens
Collateral: ${formatAmount(l.collateral, 9)} SOL
Upfront fee paid: ${formatAmount(l.termFee, 9)} SOL
Deadline: ${new Date(Number(l.deadline) * 1000).toISOString()}
Grace ends: ${new Date(Number(l.defaultAt) * 1000).toISOString()}`,
    );
  } catch {
    text("loan-detail", "");
  }
};
async function exitCommon() {
  const { c, loan, pool } = await loanContext();
  return {
    c,
    loan,
    pool,
    accounts: {
      actor: signer(),
      config: await sw.configAddress(),
      policy: loan.data.policy,
      market: loan.data.market,
      pool: loan.data.pool,
      loan: loan.address,
      exposure: await sw.exposureAddress(loan.data.pool, loan.data.borrower),
      borrower: loan.data.borrower,
      eventAuthority: await sw.eventAuthority(),
      program: sw.PROGRAM,
    },
  };
}
el("margin").onclick = () =>
  void task(async () => {
    const { accounts } = await exitCommon();
    const amount = rawAmount(input("margin-amount"), 9);
    if (amount === 0n) throw new Error("Enter positive collateral");
    const instruction = await sw.getAddMarginInstructionAsync({
      ...accounts,
      amount,
    });
    await review(
      `Add ${formatAmount(amount, 9)} SOL collateral. This does not extend the deadline or refund the upfront fee.`,
      [instruction],
    );
  });
el("repay").onclick = () =>
  void task(async () => {
    const { c, loan, accounts } = await exitCommon();
    const instruction = await sw.getRepayInstructionAsync({
      ...accounts,
      mint: c.mint,
      tokenVault: await sw.vaultAddress(loan.data.pool),
      tokenProgram: c.mintInfo.tokenProgram,
      source: address(input("token-account")),
    });
    await review(
      `Repay the full ${formatAmount(loan.data.q, c.mintInfo.decimals)} borrowed tokens from your token account. Remaining collateral goes to the borrower. The fixed upfront fee stays paid.`,
      [instruction],
    );
  });
el("close-route-request").onclick = () =>
  void task(async () => {
    const { c, loan } = await loanContext();
    const execution = await sw.executionAddresses(loan.address);
    text(
      "close-hint",
      `Buy at least ${loan.data.q} raw units of ${c.mint} with an ExactIn WSOL route.
Taker: ${execution.executionAuthority}
Destination: ${execution.executionBase}
Input chosen below must guarantee full token debt. No partial close.`,
    );
  });
el("close").onclick = () =>
  void task(async () => {
    const { c, loan, accounts } = await exitCommon();
    const execution = await sw.executionAddresses(loan.address);
    const inputAmount = rawAmount(input("buy-input"), 9);
    const maxBuybackCost = rawAmount(input("max-buyback"), 9);
    if (inputAmount > maxBuybackCost)
      throw new Error("Buyback input exceeds your signed maximum");
    const binding: sw.RouteBinding = {
      mint: c.mint,
      tokenProgram: c.mintInfo.tokenProgram,
      side: "buy",
      quantity: loan.data.q,
      inputAmount,
      minimumOutput: loan.data.q,
      setupLamports: rawAmount(input("setup-cap"), 9),
      ...execution,
    };
    const built = await routeBuild(binding, loan.address, loan.data.borrower);
    const actualInput = boundBuyInput(built.inAmount, inputAmount);
    const route = await sw.planBundle(built, {
      ...binding,
      inputAmount: actualInput,
    });
    const instruction = await sw.getClosePositionInstructionAsync({
      ...accounts,
      mint: c.mint,
      tokenVault: await sw.vaultAddress(loan.data.pool),
      tokenProgram: c.mintInfo.tokenProgram,
      ...execution,
      maxBuybackCost,
      temporaryFunding: rawAmount(input("temporary-cap"), 9),
      route: route.route,
    });
    const tables = await sw.readLookupTables(
      input("rpc"),
      route.lookupTableKeys,
    );
    await review(
      `Buy back all ${formatAmount(loan.data.q, c.mintInfo.decimals)} tokens. Maximum buyback cost: ${formatAmount(maxBuybackCost, 9)} SOL. Separately paid native DEX setup allowance: up to ${formatAmount(binding.setupLamports??0n, 9)} SOL; spent setup is not refunded. Remaining collateral returns to the borrower. The upfront fee is not refunded.`,
      [sw.appendRoute(instruction, route)],
      tables,
    );
  });
el("cancel").onclick = invalidate;
el("sign").onclick = () =>
  void task(async () => {
    const reviewed = pending;
    if (!reviewed) throw new Error("Prepare a fresh transaction first");
    if (
      connection !== reviewed.connection ||
      input("rpc") !== reviewed.url ||
      input("chain") !== reviewed.chain
    )
      throw new Error("Connection changed; prepare again");
    invalidate();
    status("Waiting for wallet approval…");
    el("review").hidden = false;
    el<HTMLButtonElement>("sign").disabled = true;
    const signingRevision = intent.capture();
    assertConnection(reviewed.connection, reviewed.chain);
    const signed = await intent.guard(
      signingRevision,
      signTransaction(
        reviewed.connection,
        reviewed.prepared.bytes,
        reviewed.chain,
      ),
    );
    const locallyDerived = sw.signedTransactionSignature(signed);
    if (connection !== reviewed.connection)
      throw new Error("Wallet changed; reconnect and prepare again");
    unresolved = {
      signature: locallyDerived,
      url: reviewed.url,
      lastValidBlockHeight: reviewed.prepared.lastValidBlockHeight,
      broadcastStarted: false,
    };
    status(`Signed as ${locallyDerived}. Submitting exactly the reviewed transaction…`);
    let signature: Signature;
    try {
      signature = await sw.submitSigned(
        reviewed.url,
        reviewed.prepared,
        signed,
        (derived) => {
          if (derived !== locallyDerived)
            throw new Error("Locally derived transaction signature changed");
          intent.assert(signingRevision);
          if (connection !== reviewed.connection)
            throw new Error("Wallet changed; reconnect and prepare again");
          assertConnection(reviewed.connection, reviewed.chain);
          if (unresolved?.signature === derived)
            unresolved = { ...unresolved, broadcastStarted: true };
        },
      );
    } catch (e) {
      if (
        unresolved?.signature === locallyDerived &&
        (!unresolved.broadcastStarted || e instanceof sw.TransactionFailedError)
      )
        unresolved = undefined;
      throw e;
    }
    unresolved = undefined;
    try {
      await load();
      status(`Confirmed signature: ${signature}\nPool and wallet balances refreshed.`);
    } catch (refreshError) {
      status(`Confirmed signature: ${signature}\nTransaction completed. Refresh the pool to update balances: ${refreshError instanceof Error ? refreshError.message : "RPC unavailable"}`);
    }
  }, true);
el("connect").onclick = () =>
  void task(async () => {
    stopWalletWatch();
    stopWalletWatch = () => {};
    connection = undefined;
    const wallet = wallets[Number(input("wallet"))];
    if (!wallet)
      throw new Error(
        "Install a wallet with Wallet Standard transaction signing, then reload this page",
      );
    const connected = await connectWallet(wallet);
    intent.assert(activeRevision);
    assertConnection(connected);
    connection = connected;
    stopWalletWatch = watchWallet(connected, () => {
      if (connection !== connected) return;
      invalidate();
      connection = undefined;
      context = undefined;
      ownedLoans = [];
      text("connect", "Reconnect wallet");
      status(
        "Wallet accounts or features changed. Reconnect and prepare a fresh transaction.",
      );
    });
    text(
      "connect",
      connected.address.slice(0, 5) + "…" + connected.address.slice(-4),
    );
    status(
      "Connected " + connected.address + ". No transaction has been sent.",
    );
    if (input("mint")) await load();
  });
const localDemoEnabled = installLocalDemoWallet({
  enabled: import.meta.env.VITE_ENABLE_LOCAL_DEMO === "true",
  hostname: () => window.location.hostname,
  rpcUrl: () => input("rpc"),
  serviceUrl: () => input("service-url"),
});
if (localDemoEnabled) {
  el("local-demo-note").hidden = false;
  el<HTMLInputElement>("mint").value = import.meta.env.VITE_DEMO_MINT?.trim() || "";
  for (const [id, value] of Object.entries({
    quantity: "100", "min-sale": "0.000001", "max-margin": "1", "max-fee": "0.1",
    "buy-input": "0.3", "max-buyback": "0.3",
  })) el<HTMLInputElement>(id).value = value;
}
discoverWallets(() => {
  if (connection && !wallets.includes(connection.wallet)) {
    invalidate();
    connection = undefined;
    context = undefined;
    ownedLoans = [];
    stopWalletWatch();
    stopWalletWatch = () => {};
    text("connect", "Reconnect wallet");
    status("Wallet was removed. Reconnect and prepare a fresh transaction.");
  }
  const select = el<HTMLSelectElement>("wallet");
  select.replaceChildren();
  wallets.forEach((w, i) => select.add(new Option(w.name, String(i))));
  if (!wallets.length) select.add(new Option("No wallet detected", ""));
});
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-tab]"))
  button.onclick = () => {
    for (const b of document.querySelectorAll("[data-tab]")) {
      b.classList.toggle("active", b === button);
      b.setAttribute("aria-selected", String(b === button));
      b.setAttribute("tabindex", b === button ? "0" : "-1");
    }
    for (const panel of document.querySelectorAll<HTMLElement>(".panel"))
      panel.hidden = panel.id !== button.dataset.tab;
  };
const actionTabs = [...document.querySelectorAll<HTMLButtonElement>("[data-tab]")];
for (const [index, button] of actionTabs.entries())
  button.addEventListener("keydown", (event) => {
    const next = event.key === "ArrowRight" ? (index + 1) % actionTabs.length
      : event.key === "ArrowLeft" ? (index + actionTabs.length - 1) % actionTabs.length
      : event.key === "Home" ? 0 : event.key === "End" ? actionTabs.length - 1 : undefined;
    if (next === undefined) return;
    event.preventDefault();
    actionTabs[next].click();
    actionTabs[next].focus();
  });
document.querySelector<HTMLAnchorElement>('a[href="#protocol-details"]')?.addEventListener("click", () => {
  el<HTMLDetailsElement>("protocol-details").open = true;
});
document.addEventListener("input", invalidate);
for (const id of ["rpc", "mint", "generation"])
  el(id).addEventListener("input", () => {
    context = undefined;
    ownedLoans = [];
  });
