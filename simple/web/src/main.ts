import "./style.css";
import {
  address,
  createNoopSigner,
  createSolanaRpc,
  getBase58Encoder,
  type Address,
  type Instruction,
  type AddressesByLookupTableAddress,
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
import { IntentRevision } from "./intent.ts";
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
 <header class="top"><div class="brand">short<b>.win</b> <span class="badge">Pooled · local development</span></div><div class="wallet"><select id="wallet" aria-label="Wallet"></select><button id="connect">Connect wallet</button></div></header>
 <section class="hero"><div class="eyebrow">One pool per eligible token</div><h1>Lend your tokens.<br>Take the other side.</h1><p>Lenders share actually paid SOL fees. Shorts sell and buy back through Jupiter, with SOL collateral and a fixed upfront term fee.</p></section>
 <div class="notice">Independent development version; release qualification and operator setup remain pending. The owner’s plan is borrowing-capped mainnet before external audit and bug bounty, funded later from fees. Borrowing caps apply per market; deposits and the number of markets are uncapped. Prices and completed history are trusted assertions from the protocol’s in-house service.</div>
 <details class="settings"><summary>Connection settings</summary><div class="fields"><div><label for="rpc">Solana RPC</label><input id="rpc" value="http://127.0.0.1:8899" spellcheck="false"></div><div><label for="chain">Wallet signing chain</label><select id="chain"><option value="solana:devnet">Devnet / local validator wallet</option><option value="solana:mainnet">Mainnet</option><option value="solana:testnet">Testnet</option></select></div></div><p class="muted">The loaded config’s genesis hash must match the RPC before any transaction is prepared. Local validator signing uses the wallet’s devnet chain; the reviewed transaction goes to the RPC shown above.</p><label for="service-url">In-house quote and history service</label><input id="service-url" value="http://127.0.0.1:8787" spellcheck="false"><p class="muted">The configured operator must run this bridge and its funded reporter. A request warms exact-quantity history; it does not guarantee immediate availability.</p><div class="muted" id="program"></div></details>
 <section class="card"><h2>Choose a token</h2><div class="fields"><div><label for="mint">Token mint address</label><input id="mint" placeholder="Paste a Solana mint address" spellcheck="false"></div><div><label for="generation">Pool generation</label><input id="generation" placeholder="Current pool (leave blank)" inputmode="numeric"></div></div><div class="actions"><button id="load">Load pool</button><button class="secondary" id="register">Prepare registration</button></div><p class="muted" id="eligibility">Initialized SPL and eligible Token-2022 mints without a freeze authority are supported. Metadata-only extensions are allowed; transfer fees, hooks and other token-behavior extensions are excluded. WSOL is collateral. Registration does not mean a token is ready for shorting: finite price history and a full trade route must qualify.</p></section>
 <div class="grid"><section class="card"><div class="stats"><div class="stat"><small>Available tokens</small><strong id="cash">—</strong></div><div class="stat"><small>Tokens borrowed</small><strong id="debt">—</strong></div><div class="stat"><small>Paid SOL reserve</small><strong id="income">—</strong></div></div>
 <nav class="tabs" aria-label="Actions"><button class="active" data-tab="lend">Lend</button><button data-tab="short">Short</button><button data-tab="positions">Positions</button></nav>
 <section class="panel" id="lend"><label for="token-account">Your token account</label><input id="token-account" placeholder="Connected wallet’s associated token account" spellcheck="false"><div class="fields"><div><label for="deposit-amount">Tokens to lend</label><input id="deposit-amount" inputmode="decimal" placeholder="0.0"></div><div><label for="withdraw-shares">Pool shares to redeem</label><input id="withdraw-shares" inputmode="decimal" placeholder="0.0"></div></div><div class="actions"><button id="deposit">Prepare deposit</button><button class="secondary" id="withdraw">Prepare withdrawal</button><button class="secondary" id="claim">Prepare SOL claim</button></div><p class="muted" id="receipt">Connect a wallet and load the pool to view your shares and whole-lamport SOL claim.</p><p class="muted">Withdrawals use available token cash. The final shares cannot exit while a loan remains. Share changes settle paid SOL first; fractions stay while shares remain. A full receipt exit clears its sub-lamport fraction, and the final holder with no debt receives tracked SOL dust.</p></section>
 <section class="panel" id="short" hidden><div class="fields"><div><label for="quantity">Token quantity to short</label><input id="quantity" inputmode="decimal" placeholder="0.0"></div><div><label for="min-sale">Minimum sale proceeds (SOL)</label><input id="min-sale" inputmode="decimal" placeholder="0.0"></div><div><label for="max-margin">Maximum collateral contribution (SOL)</label><input id="max-margin" inputmode="decimal" placeholder="0.0"></div><div><label for="max-fee">Maximum upfront term fee (SOL)</label><input id="max-fee" inputmode="decimal" placeholder="0.0"></div></div><p class="muted" id="terms">Load the pool’s immutable policy to view its term and fee.</p><div class="actions"><button class="secondary" id="warm-reference">Request price-history warm-up</button><button class="secondary" id="route-request">Show advanced route addresses</button></div><pre class="status" id="route-hint"></pre><div class="actions"><button id="open">Prepare short</button></div></section>
 <section class="panel" id="positions" hidden><label for="loans">Your positions in this market</label><select id="loans"><option value="">Load a pool and connect a wallet</option></select><pre class="loan-detail" id="loan-detail"></pre><label for="margin-amount">Additional SOL collateral</label><input id="margin-amount" inputmode="decimal" placeholder="0.0"><div class="actions"><button id="margin">Prepare collateral addition</button><button class="secondary" id="repay">Prepare full token repayment</button></div><hr class="separator"><div class="fields"><div><label for="buy-input">Jupiter buyback input (SOL)</label><input id="buy-input" inputmode="decimal" placeholder="0.0"></div><div><label for="max-buyback">Maximum buyback cost (SOL)</label><input id="max-buyback" inputmode="decimal" placeholder="0.0"></div></div><button class="secondary" id="close-route-request">Show closing route addresses</button><pre class="status" id="close-hint"></pre><div class="actions"><button id="close">Prepare full Jupiter close</button></div><p class="muted">Only full token repayment closes the principal debt. A Jupiter close must return every borrowed token; partial fills do not close a loan.</p></section>
 </section><aside><section class="card"><h2>Before you act</h2><p class="muted">The fixed term fee is paid upfront in SOL, split between lenders and the protocol. Closing early does not refund unused time.</p><p class="muted">If full token repayment remains impossible after the deadline and its 24-hour grace period, remaining collateral compensates lenders after the capped keeper fee. The borrower receives zero economic payout after grace, even if a recent price appears healthy. Account rent is separate.</p><p class="muted">Trusted pricing does not guarantee a liquid route or protect against token price jumps. “Unknown” or warming history prevents new shorts. Lending tokens does not guarantee immediate withdrawal.</p><details><summary>Local development route and spending settings</summary><label for="route-bundle">Jupiter V2 build response JSON</label><textarea id="route-bundle" spellcheck="false" placeholder="Paste a real ExactIn V2 build response for the displayed protocol scratch authority"></textarea><p class="muted">Normally the in-house service supplies the route. A fresh real build response may be pasted here for advanced local testing. The SDK binds its mints, whole-position quantity and scratch endpoints. Setup, cleanup and tip instructions are excluded.</p><label for="rent-cap">Maximum new account rent (SOL)</label><input id="rent-cap" value="0.1" inputmode="decimal"><label for="creation-fee-cap">Maximum market creation fee (SOL)</label><input id="creation-fee-cap" value="0.1" inputmode="decimal"><label for="temporary-cap">Maximum total swap advance (SOL)</label><input id="temporary-cap" value="0.01" inputmode="decimal"><label for="setup-cap">Maximum native DEX setup allowance (SOL)</label><input id="setup-cap" value="0.005" inputmode="decimal"><p class="muted">DEX account setup can consume this separately paid allowance. Unused funds return to the payer; spent setup is nonrefundable and is outside collateral and the term fee. The total swap advance must cover setup plus temporary token-account deposits.</p></details></section>
 <section class="card review" id="review" hidden><h2>Review transaction</h2><p id="review-text"></p><p class="muted">Simulation uses the current RPC state. Signing can still fail if liquidity, limits, fees or blockhash validity change. Only the reviewed message will be submitted.</p><button id="sign" class="full">Review in wallet and send</button><button id="cancel" class="secondary full" style="margin-top:8px">Cancel</button></section>
 <section class="card"><h2>Activity</h2><pre class="status" id="status" role="status" aria-live="polite">Choose a token to begin. No wallet transaction is sent automatically.</pre></section></aside></div>
 <footer>New pooled program · <span id="footer-program"></span><br>This local interface is separate from the previous quote-rent version. No old-pool balances, epochs or receivables are converted.</footer></main>`;
const el = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const input = (id: string) => el<HTMLInputElement>(id).value.trim();
const text = (id: string, value: string) => {
  el(id).textContent = value;
};
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
  text("status", message);
}
function error(e: unknown) {
  status(e instanceof Error ? e.message : String(e));
}
async function task(fn: () => Promise<void>, keepReview = false) {
  if (busy) return;
  busy = true;
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
Signing chain: ${input("chain")}
Wallet: ${connection.address}
Estimated network fee: ${formatAmount(prepared.feeLamports, 9)} SOL
Simulation: ${prepared.units} compute units
Wire size: ${prepared.bytes.length} bytes`,
  );
  el("review").hidden = false;
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
  text("income", pool ? formatAmount(pool.solReserve, 9) + " SOL" : "—");
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
    if (connection !== reviewed.connection)
      throw new Error("Wallet changed; reconnect and prepare again");
    status("Submitting exactly the reviewed transaction…");
    const signature = await sw.submitSigned(
      reviewed.url,
      reviewed.prepared,
      signed,
      () => {
        intent.assert(signingRevision);
        if (connection !== reviewed.connection)
          throw new Error("Wallet changed; reconnect and prepare again");
        assertConnection(reviewed.connection, reviewed.chain);
      },
    );
    status(`Confirmed signature: ${signature}
Refresh the pool to load the resulting state.`);
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
    for (const b of document.querySelectorAll("[data-tab]"))
      b.classList.toggle("active", b === button);
    for (const panel of document.querySelectorAll<HTMLElement>(".panel"))
      panel.hidden = panel.id !== button.dataset.tab;
  };
document.addEventListener("input", invalidate);
for (const id of ["rpc", "mint", "generation"])
  el(id).addEventListener("input", () => {
    context = undefined;
    ownedLoans = [];
  });
