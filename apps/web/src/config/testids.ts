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

const p =
    <K extends string = string>(prefix: string) =>
    (key: K): string =>
        `${prefix}-${key}`;

export const T = {
    // ---- shell ----------------------------------------------------------------------------
    header: 'app-header',
    clusterBadge: 'cluster-badge',
    nav: p<'markets' | 'positions' | 'lend' | 'status' | 'security'>('nav'),
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
    page: p<'markets' | 'create-market' | 'market' | 'positions' | 'position' | 'lend' | 'status' | 'security'>('page'),
    routeStub: p('route-stub'),

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
    withdrawPercent: p<'25' | '50'>('withdraw-percent'),
    lenderEpochs: p('lender-epochs'),
    lenderIndexer: p('lender-indexer'),
    lenderClaim: p('lender-claim'),
    lenderSync: p('lender-sync'),
    lenderClaimAll: 'lender-claim-all',
    lenderEmpty: 'lender-empty',

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

    // ---- disclosures (scaffold) -----------------------------------------------------------
    disclosure: p<DisclosureId>('disclosure'),
    disclosureAck: p<DisclosureId>('disclosure-ack'),
} as const;
