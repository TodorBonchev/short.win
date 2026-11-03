import "./public.css";

const repository = "https://github.com/TodorBonchev/short.win";
const sourceRef = (import.meta.env.VITE_SOURCE_REF?.trim() || "main")
  .split("/").map(encodeURIComponent).join("/");
const sourceBase = `${repository}/blob/${sourceRef}`;
const sourceTree = `${repository}/tree/${sourceRef}`;
const localGuide = `${sourceBase}/scripts/local-fork/README.md`;
const mark = `<svg class="brand-mark" width="32" height="32" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z"/><path fill="#f0b90b" d="M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z"/></svg>`;
const films = [
  {
    file: "01-lend",
    number: "01",
    title: "Lend your tokens.",
    category: "LEND",
    duration: "24 sec",
    description: "Supply tokens, review the deposit, and receive pool shares. Share in SOL fees actually paid by borrowers.",
    detail: "Withdrawals depend on available token liquidity.",
  },
  {
    file: "02-short",
    number: "02",
    title: "Take the other side.",
    category: "SHORT",
    duration: "28 sec",
    description: "Borrow tokens and sell through Jupiter with SOL collateral, a fixed term, and spending limits you review.",
    detail: "The borrowing fee is paid upfront. Closing early does not refund it.",
  },
  {
    file: "03-manage-close",
    number: "03",
    title: "Stay in control.",
    category: "MANAGE & CLOSE",
    duration: "28 sec",
    description: "Find your position, add collateral, then review a Jupiter buyback that repays the full token debt.",
    detail: "You can also repay the exact borrowed token quantity directly.",
  },
];

const video = (file: string, label: string, landscape = false) => `<video class="film-player${landscape ? " landscape" : ""}" controls playsinline preload="none" poster="/videos/${file}-poster.jpg" aria-label="${label}"><source src="/videos/${file}.mp4" type="video/mp4">Your browser does not support embedded video. <a href="/videos/${file}.mp4">Download this walkthrough</a>.</video>`;

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `
<a class="skip-link" href="#workflows">Skip to workflows</a>
<div class="site-shell">
  <header class="site-header">
    <a class="brand" href="#" aria-label="short.win home">${mark}short<span>.win</span></a>
    <nav class="header-nav" aria-label="Main navigation"><a href="#workflows">Workflows</a><a href="#terms">How it works</a><a href="${sourceTree}">Source <span aria-hidden="true">↗</span></a></nav>
    <span class="demo-badge"><span aria-hidden="true"></span>Local demo showcase</span>
  </header>
  <main>
    <section class="hero" aria-labelledby="hero-title">
      <div class="hero-copy"><p class="eyebrow"><span aria-hidden="true"></span>A different side of the market</p><h1 id="hero-title">Put your tokens<br>to <em>work.</em></h1><p class="hero-description">Pooled token lending. SOL collateral.<br>Clear terms on both sides of the trade.</p><div class="hero-actions"><a class="button primary" href="#workflows">Watch the workflows <span aria-hidden="true">↓</span></a><a class="button secondary" href="${localGuide}">Run locally <span aria-hidden="true">↗</span></a></div><p class="hero-footnote">Recorded workflows · Experimental software</p></div>
      <div class="protocol-visual" aria-hidden="true"><div class="visual-orbit"></div><div class="visual-orbit second"></div><div class="visual-label label-lend">LEND & EARN <span>↗</span></div><div class="visual-mark">${mark}</div><div class="visual-label label-short"><span>↘</span> BORROW & SHORT</div><div class="visual-caption">TWO SIDES. ONE POOL.</div></div>
    </section>
    <aside class="demo-notice" aria-label="About this demonstration"><span class="notice-icon" aria-hidden="true">i</span><p><strong>Real Jupiter execution. A local test environment.</strong> The recordings use genuine Jupiter Router V2 and historical DEX account snapshots on an isolated local validator, with test funds and synthetic price history. Wallet workflows run locally.</p><a href="${localGuide}">Demo details <span aria-hidden="true">↗</span></a></aside>
    <section class="workflows" id="workflows" aria-labelledby="workflows-title">
      <div class="section-heading"><div><p class="section-kicker">01 / SEE IT IN ACTION</p><h2 id="workflows-title">From first deposit to full close.</h2></div><p>Four short walkthroughs.<br>Every step, in the app.</p></div>
      <div class="film-grid">${films.map((film) => `<article class="film-card"><div class="film-topline"><span>${film.number} / ${film.category}</span><span>${film.duration}</span></div>${video(film.file, `${film.category.toLowerCase()} workflow recorded in the local test environment`)}<div class="film-copy"><h3>${film.title}</h3><p>${film.description}</p><p class="film-detail">${film.detail}</p></div></article>`).join("")}</div>
      <article class="overview-card"><div class="overview-media">${video("04-overview", "Complete short.win overview recorded in the local test environment", true)}</div><div class="overview-copy"><p class="section-kicker">04 / THE COMPLETE PICTURE · 45 SEC</p><h3>One pool.<br>Two ways to participate.</h3><p>See how lending, borrowing, and closing connect in a single workflow.</p><p class="film-detail">The films combine successful local takes. Balances and position IDs can differ between scenes. Historical accounts span several captured slots.</p></div></article>
    </section>
    <section class="terms-section" id="terms" aria-labelledby="terms-title"><div class="section-heading"><div><p class="section-kicker">02 / KNOW YOUR TERMS</p><h2 id="terms-title">Understand the mechanics.</h2></div><a class="text-link" href="${sourceBase}/docs/SPEC.md">Read the specification <span aria-hidden="true">↗</span></a></div><div class="terms-grid"><article><span class="term-icon" aria-hidden="true">↗</span><h3>Token shares. Paid SOL fees.</h3><p>Lenders receive token-denominated pool shares and can claim their portion of SOL fees paid into the pool. Withdrawals require available token liquidity.</p></article><article><span class="term-icon" aria-hidden="true">◷</span><h3>A fixed term, paid upfront.</h3><p>Borrowers post SOL collateral, borrow a token quantity, and sell through Jupiter. The term fee is fixed upfront and remains paid if the position closes early.</p></article><article><span class="term-icon" aria-hidden="true">↘</span><h3>Close the whole token debt.</h3><p>A full close buys back every borrowed token through Jupiter. Borrowers can add collateral or repay in tokens. Defaults can lose token principal and return SOL compensation.</p></article></div></section>
    <section class="local-section" aria-labelledby="local-title"><div><p class="section-kicker">03 / EXPLORE THE SOURCE</p><h2 id="local-title">Make it your local workspace.</h2><p>The repository includes the program, client, service, and local demo setup. Follow the guide to start the validator and app on your machine.</p></div><div class="local-actions"><a class="button primary" href="${localGuide}">Read the setup guide <span aria-hidden="true">↗</span></a><a class="text-link local-app-link" href="http://127.0.0.1:5173">Open your running local app <span aria-hidden="true">↗</span></a><small>Requires the demo running on this device at 127.0.0.1:5173.</small></div></section>
    <p class="risk-note">Experimental software. Release qualification, an external audit, and a bug bounty remain pending. Trusted price-history reports do not guarantee an executable trade route or protect against token price jumps. Review the <a href="${sourceBase}/SECURITY.md">security policy</a> and <a href="${sourceBase}/docs/SCOPE.md">protocol scope</a>.</p>
  </main>
  <footer><a class="brand footer-brand" href="#" aria-label="short.win home">${mark}short<span>.win</span></a><span>Choose your side. Know your terms.</span><a href="${sourceTree}">View on GitHub <span aria-hidden="true">↗</span></a></footer>
</div>`;
