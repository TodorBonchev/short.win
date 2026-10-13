/**
 * The data-testid contract (FRONTEND.md §11). Components take their ids from here and the
 * Playwright specs (apps/web/e2e) select by the same constants, so a renamed id breaks the type
 * check rather than an E2E run. Adding an id is free; renaming or removing one is a contract change
 * (update FRONTEND.md §11 in the same change).
 *
 * Conventions: kebab-case; a parametrised id appends `-<key>` where the key is a base58 address,
 * a signature, an enum value or a disclosure id. State is exposed through data attributes on the
 * same element (`data-state`, `data-stage`, `data-ok`, `data-code`, `data-verdict`, `data-risk`,
 * `data-value` with the raw integer), never through the text alone.
 */
import type { DisclosureId } from './disclosures.ts';
import type { GlossaryId } from './glossary.ts';

const p =
    <K extends string = string>(prefix: string) =>
    (key: K): string =>
        `${prefix}-${key}`;

export const T = {
    // ---- shell ----------------------------------------------------------------------------
    header: 'app-header',
    clusterBadge: 'cluster-badge',
    nav: p<'markets' | 'positions' | 'lend' | 'status' | 'security' | 'how-it-works'>('nav'),
    searchInput: 'search-input',
    searchResults: 'search-results',
    searchResult: p('search-result'),
    walletButton: 'wallet-button',
    walletMenu: 'wallet-menu',
    walletOption: p('wallet-option'),
    walletAddress: 'wallet-address',
    walletDisconnect: 'wallet-disconnect',
    /** On a wallet entry whose name another registered wallet also uses (M9-SEC-8). */
    walletDuplicate: 'wallet-duplicate-badge',
    /** On a wallet entry that can only sign-and-send (no check before sending, §5.4). */
    walletSendsItself: 'wallet-sends-itself-badge',
    /** Shown instead of the app when it is framed by another page (§9.3). */
    frameGuard: 'frame-guard',
    /** A preview's chain-read age when it exceeds 2 slots (§6.6); `data-slots`, `data-state`. */
    slotAge: 'slot-age-badge',
    statusBar: 'status-bar',
    statusIndexer: 'status-indexer',
    statusRpc: 'status-rpc',
    statusWs: 'status-ws',
    statusSolUsd: 'status-sol-usd',
    statusMath: 'status-math',
    staleBadge: 'stale-badge',
    fallbackBanner: 'fallback-banner',
    toastRegion: 'toast-region',
    toast: 'toast',
    /** The root element of every page; tracks keep it on their page (router smoke test, E2E). */
    page: p<
        | 'markets'
        | 'create-market'
        | 'market'
        | 'positions'
        | 'position'
        | 'lend'
        | 'status'
        | 'security'
        | 'how-it-works'
    >('page'),
    routeStub: p('route-stub'),
    // appended by the responsive foundation (M10-RESP; FRONTEND.md §7.6, §11.1)
    /** The phone tab bar's fourth cell: opens the More sheet (`aria-expanded`). */
    navMore: 'nav-more',
    moreSheet: 'more-sheet',
    moreStatus: 'more-status',
    moreSecurity: 'more-security',
    moreCreateMarket: 'more-create-market',
    moreSolUsd: 'more-sol-usd',
    /** Under 1024 px: the header's Search button, and the overlay's Cancel. */
    searchToggle: 'search-toggle',
    searchClose: 'search-close',
    /** The phone's status strip: the worst state in words; `data-state` ok | warn | bad. */
    statusSummary: 'status-summary',
    notFoundCta: 'not-found-cta',
    /** A DataTable's sort select / direction button (card mode), `<id>` = the table's test id. */
    tableSort: p('table-sort'),
    tableSortDir: p('table-sort-dir'),
    /** An enabled "Connect wallet" where a form or page needs a wallet. */
    connectCta: p<'short' | 'lend' | 'positions' | 'lend-dashboard' | 'position'>('connect-cta'),
    /** A truncated value's full-text popover (mint, address, wallet). */
    reveal: p('reveal'),
    /** No wallet on a phone: open this page in a wallet app's browser (universal link). */
    walletOpenIn: p<'backpack' | 'phantom' | 'solflare'>('wallet-open-in'),
    walletInstall: p<'backpack' | 'phantom' | 'solflare'>('wallet-install'),
    walletReload: 'wallet-reload',
    walletCopyAddress: 'wallet-copy-address',
    walletExplorer: 'wallet-explorer',
    walletMyPositions: 'wallet-my-positions',
    walletMyLending: 'wallet-my-lending',
    /** The header wallet popup's ✕ (a phone's bottom sheet; hidden from 640 px up). */
    walletSheetClose: 'wallet-sheet-close',
    // appended by the mobile wallets (O-6, 2026-10-13; FRONTEND.md §5.4 (6), §11.1)
    /** A mobile device with wallets listed: the open-in links after the entries (outside the menu). */
    walletOpenElsewhere: 'wallet-open-elsewhere',
    /** A social app's in-app browser without a wallet: "Open this page in your browser" and Copy link. */
    walletInAppBrowser: 'wallet-in-app-browser',
    walletCopyLink: 'wallet-copy-link',
    /** The Mobile Wallet Adapter entry's second line (or the local-network permission text). */
    walletMwaHint: 'wallet-mwa-hint',

    // ---- markets table (MARKETS) ----------------------------------------------------------
    marketsTable: 'markets-table',
    marketsRow: p('markets-row'),
    marketsFilterVenue: 'markets-filter-venue',
    marketsFilterTier: 'markets-filter-tier',
    marketsSort: p<'tvl' | 'oi' | 'util' | 'created' | 'open_positions'>('markets-sort'),
    marketsEmpty: 'markets-empty',
    marketsLoadMore: 'markets-load-more',
    marketsCreateLink: 'markets-create-link',
    marketsSearch: 'markets-search',
    marketsFilterActive: 'markets-filter-active',
    marketsOpenMint: 'markets-open-mint',

    // ---- market page (MARKETS) ------------------------------------------------------------
    marketHeader: 'market-header',
    marketSymbol: 'market-symbol',
    marketMint: 'market-mint',
    marketTier: 'market-tier-badge',
    marketVenue: 'market-venue-badge',
    marketFlag: p<
        | 'paused-opens'
        | 'coverage-paused'
        | 'route-broken'
        | 'venue-upgraded'
        | 'migrating'
        | 'graduation-halted'
        | 'list-only'
        | 'mint-unsafe'
        | 'warming-up'
        | 'pool-window'
    >('market-flag'),
    marketStat: p<'inventory' | 'utilisation' | 'rate' | 'oi' | 'caps' | 'tvl' | 'price' | 'lenders'>('market-stat'),
    marketTierWhy: 'market-tier-why',
    marketCreatorDeposits: 'market-creator-deposits',
    marketLiquidations: 'market-liquidations-table',
    marketLiquidationRow: p('market-liquidation-row'),
    marketTab: p<'overview' | 'positions' | 'lenders' | 'liquidations' | 'pools' | 'coverage' | 'events'>('market-tab'),
    marketPanelTab: p<'short' | 'lend'>('market-panel-tab'),
    marketSource: 'market-source',
    marketNotFound: 'market-not-found',
    marketBanner: p('market-banner'),
    marketSpotPrice: 'market-spot-price',
    /** The indexer named another token for this mint; the page shows the chain's view (M9-SEC-3). */
    marketIndexerMismatch: 'market-indexer-mismatch',
    /** Another listed market uses this symbol (R-MINT §10.11). */
    marketDuplicateSymbol: 'market-duplicate-symbol',
    marketFees: 'market-fees-table',
    marketLenderLoss: 'market-lender-loss',
    marketPositions: 'market-positions-table',
    marketPositionRow: p('market-position-row'),
    marketLenders: 'market-lenders-table',
    marketLenderRow: p('market-lender-row'),
    marketCreatorShare: 'market-creator-share',
    marketPools: 'market-pools-table',
    marketCoverage: 'market-coverage',
    marketEvents: 'market-events-table',
    marketRouteChain: p<'no-route' | 'pool-fail'>('market-route-chain'),

    // ---- create market (MARKETS) ----------------------------------------------------------
    createMintInput: 'create-mint-input',
    createVerdict: 'create-check-verdict',
    createReason: p('create-check-reason'),
    createVenue: 'create-venue-detected',
    createPools: 'create-pool-candidates',
    createPoolRow: p('create-pool-row'),
    createCost: 'create-cost',
    createSubmit: 'create-submit',
    createExisting: 'create-existing-market',
    createMintInfo: 'create-mint-info',
    createChoice: p('create-choice'),

    // ---- lend (LEND) ----------------------------------------------------------------------
    lendPanel: 'lend-panel',
    lendTab: p<'deposit' | 'withdraw' | 'claim'>('lend-tab'),
    lendAmountInput: 'lend-amount-input',
    lendMax: 'lend-max',
    lendTokenAccount: 'lend-token-account-select',
    lendPreview: p<'shares' | 'share-bps' | 'value' | 'unpaid-rent' | 'unpaid-rent-share'>('lend-preview'),
    lendSubmit: 'lend-submit',
    lendCpiGuardNotice: 'lend-cpi-guard-notice',
    lendCpiGuardBracket: 'lend-cpi-guard-bracket',
    withdrawSharesInput: 'withdraw-shares-input',
    withdrawMax: 'withdraw-max',
    withdrawPreview: p<'tokens' | 'sol' | 'allowance' | 'lock' | 'lock-after' | 'lock-until' | 'vesting-forfeit'>(
        'withdraw-preview',
    ),
    withdrawMemoNotice: 'withdraw-memo-notice',
    withdrawSubmit: 'withdraw-submit',
    claimValue: p<'claimable' | 'vesting' | 'pot' | 'deficit'>('claim-value'),
    claimSubmit: 'claim-submit',
    coverDeficitSubmit: 'cover-deficit-submit',
    lenderDashboard: 'lender-dashboard',
    lenderRow: p('lender-row'),
    lenderCell: p<'shares' | 'value' | 'claimable' | 'vesting' | 'lock' | 'lock-until' | 'creator'>('lender-cell'),
    // appended by LEND (M9 build; FRONTEND.md §11.3)
    lendBlocked: 'lend-blocked-reason',
    lendPosition: p<'shares' | 'share-bps' | 'value' | 'claimable' | 'lock'>('lend-position'),
    lendSlippageInput: 'lend-slippage-input',
    lendSweepNotice: 'lend-sweep-notice',
    lendSyncSubmit: 'lend-sync-submit',
    withdrawDestination: 'withdraw-destination-select',
    withdrawPercent: p<'25' | '50' | '75'>('withdraw-percent'),
    lenderEpochs: p('lender-epochs'),
    lenderIndexer: p('lender-indexer'),
    lenderClaim: p('lender-claim'),
    lenderSync: p('lender-sync'),
    lenderClaimAll: 'lender-claim-all',
    lenderEmpty: 'lender-empty',

    // ---- lend responsive (LEND, M10-RESP) -------------------------------------------------
    /** Why the form's submit is disabled (FormFoot; empty while it is enabled). */
    lendDepositReason: 'lend-deposit-reason',
    lendWithdrawReason: 'lend-withdraw-reason',
    lendClaimReason: 'lend-claim-reason',
    /** "≈ N TOKEN" under the withdraw shares input (display only). */
    withdrawApprox: 'withdraw-approx',
    /** The dashboard's empty state: Browse markets. */
    lendEmptyCta: 'lend-empty-cta',
    /** The dashboard's KPI strip. */
    lenderKpi: p<'markets' | 'value' | 'claimable' | 'credited'>('lender-kpi'),
    /** The dashboard's data source while the indexer is unavailable (chain only). */
    lenderSource: 'lender-source',
    /** Per-row shortcuts to the market's lend panel (`sheet=trade` on a phone / tablet). */
    lenderWithdraw: p('lender-withdraw'),
    lenderDeposit: p('lender-deposit'),
    /** Why a row's Claim is disabled, in text. */
    lenderClaimReason: p('lender-claim-reason'),

    // ---- short (SHORT) --------------------------------------------------------------------
    shortPanel: 'short-panel',
    shortQInput: 'short-q-input',
    shortSlippageInput: 'short-slippage-input',
    shortPreview: p<
        | 'q'
        | 'proceeds'
        | 'margin'
        | 'open-fee'
        | 'total-cost'
        | 'cr-open'
        | 'cr-ema'
        | 'liq-price'
        | 'rent-rate'
        | 'min-rent'
        | 'max-duration'
        | 'deadline'
        | 'tier-terms'
        | 'caps'
    >('short-preview'),
    shortBlocked: 'short-blocked-reason',
    shortCapWarning: 'short-cap-warning',
    shortSubmit: 'short-submit',
    positionsTable: 'positions-table',
    positionsFilter: 'positions-filter',
    positionRow: p('position-row'),
    positionCell: p<'market' | 'q' | 'vault' | 'cr' | 'liq-price' | 'pnl' | 'rent' | 'time-left' | 'risk' | 'status'>(
        'position-cell',
    ),
    positionAddMargin: 'position-add-margin',
    positionClose: 'position-close',
    positionDetail: 'position-detail',
    /** The indexer's market for a position disagrees with the chain's (M9-SEC-2). */
    positionDataMismatch: 'position-data-mismatch',
    addMarginInput: 'add-margin-input',
    addMarginPreviewCr: 'add-margin-preview-cr',
    addMarginSubmit: 'add-margin-submit',
    closeSlippageInput: 'close-slippage-input',
    closePreview: p<'buyback' | 'rent' | 'payout' | 'pnl'>('close-preview'),
    closeStraddleNotice: 'close-straddle-notice',
    closeSubmit: 'close-submit',
    // appended by TRADE (M10-RESP; FRONTEND.md §7.6, §11.4)
    /** Why the Short submit is disabled, one line (`data-reason` while there is one). */
    shortSubmitReason: 'short-submit-reason',
    /** The positions page's totals line: count, vault, P&L, rent due. */
    positionsTotals: 'positions-totals',
    positionsEmptyCta: 'positions-empty-cta',
    positionNotFoundCta: 'position-not-found-cta',
    /** A position's action bar (under 1024 px): opens the Manage sheet at that section. */
    positionActionAddMargin: 'position-action-add-margin',
    positionActionClose: 'position-action-close',
    manageSheetClose: 'manage-sheet-close',
    addMarginReason: 'add-margin-reason',
    closeReason: 'close-reason',

    // ---- protocol status (MARKETS) --------------------------------------------------------
    statusPage: 'status-page',
    statusIndexerLag: 'status-indexer-lag',
    statusPausedOpens: 'status-paused-opens',
    statusVenues: 'status-venues-table',
    statusVenueRow: p('status-venue-row'),
    statusDegraded: 'status-degraded-markets',
    statusDegradedRow: p('status-degraded-row'),
    statusRouteBroken: 'status-route-broken',
    statusRouteBrokenRow: p('status-route-broken-row'),
    statusScanned: 'status-scanned',
    statusIndexerHealth: 'status-indexer-health',
    statusCoverageGap: 'status-coverage-gap',
    statusExtras: 'status-extras-table',

    // ---- responsive, MARKETS pages (M10-RESP; FRONTEND.md §7.6, §11.2) -----------------------
    /** Phone: the markets list's filter group toggle ("Filters · n", `aria-expanded`). */
    marketsFiltersToggle: 'markets-filters-toggle',
    /** An empty filtered list: back to every market. */
    marketsClearFilters: 'markets-clear-filters',
    /** Under 1024 px: the market page's action bar, opening the trade sheet at Short / Lend. */
    marketActionShort: 'market-action-short',
    marketActionLend: 'market-action-lend',
    /** The trade sheet's close button (under 1024 px). */
    tradeSheetClose: 'trade-sheet-close',
    /** Phone: the stat strip's "More stats" toggle (`aria-expanded`). */
    marketStatsMore: 'market-stats-more',
    /** The tier sheet's close button (the "Why this tier" popover). */
    marketTierWhyClose: 'market-tier-why-close',
    /** Coarse pointer: paste the mint from the clipboard. */
    createMintPaste: 'create-mint-paste',
    /** Why Review is disabled (the form foot's reason line). */
    createSubmitReason: 'create-submit-reason',

    // ---- transaction review (scaffold; every flow) ----------------------------------------
    txModal: 'tx-modal',
    txStage: 'tx-stage',
    txSimStatus: 'tx-sim-status',
    txDelta: p('tx-delta'),
    txCu: 'tx-cu',
    txFee: 'tx-fee',
    txSign: 'tx-sign',
    txCancel: 'tx-cancel',
    txRetry: 'tx-retry',
    txError: 'tx-error',
    txSignature: 'tx-signature',
    txExplorer: 'tx-explorer-link',
    /** Who signs: wallet name and account; `data-value` = address, `data-sends-itself`. */
    txWallet: 'tx-wallet',
    /** The fee as the wallet signed it, when it changed the compute budget (FE-2). */
    txFinalFee: 'tx-final-fee',
    /** Retry rebuilt the plan from fresh chain state (a new quote). */
    txRebuilt: 'tx-rebuilt-notice',
    /** `<i>` = the instruction's index in the plan (compute-budget instructions excluded). */
    txInstruction: p('tx-instruction'),
    /** "Tick the acknowledgement above to sign": focuses the first unticked acknowledgement. */
    txSignHint: 'tx-sign-hint',
    /** Why a successful simulation is not offered for signing (`TxPlan.holdSign`: a claim that pays 0). */
    txSignHold: 'tx-sign-hold',
    /** After landing: View position / Open in explorer. */
    txConfirmedLink: 'tx-confirmed-link',
    /** The Mobile Wallet Adapter's notice beside Sign (the review stays open); `data-reason`. */
    txSignNotice: 'tx-sign-notice',

    // ---- disclosures (scaffold) -----------------------------------------------------------
    disclosure: p<DisclosureId>('disclosure'),
    disclosureAck: p<DisclosureId>('disclosure-ack'),

    // ---- newcomer help (the owner's thirteenth round, O-3 / O-2 / O-5; FRONTEND.md §4.3, §11.5) ----
    /** The intro strip above the markets list (dismissible; remembered in this browser). */
    marketsIntro: 'markets-intro',
    marketsIntroDismiss: 'markets-intro-dismiss',
    /** The strip's links: How it works, and its two risk sections. */
    marketsIntroLink: p<'how-it-works' | 'short-risks' | 'lend-risks'>('markets-intro-link'),
    /** A glossary entry's ⓘ beside a label (`aria-expanded`, `aria-controls`), its definition, its ✕. */
    glossaryTip: p<GlossaryId>('glossary-tip'),
    glossaryPopover: p<GlossaryId>('glossary-popover'),
    glossaryClose: 'glossary-close',
    /** Phone: the More sheet's How it works row. */
    moreHowItWorks: 'more-how-it-works',
    /** A How it works section, `<id>` = its anchor (`shorting`, `lending`, `what-can-go-wrong`, …). */
    howItWorksSection: p('how-it-works-section'),
    /** How it works: the alert (with Retry) when the GlobalConfig read fails or finds no account. */
    howItWorksConfigError: 'how-it-works-config-error',
    /** O-2: the Short panel's link to the page's coverage banner (in the trade sheet: closes it first). */
    shortCoverageLink: 'short-coverage-link',
    /** O-5: the Claim button's second line, when syncs may add to what it pays now. */
    claimSubmitSyncs: 'claim-submit-syncs',
} as const;
