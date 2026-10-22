# Who Wins When You Let People Short Memecoins?

*A game-theory teardown of my own protocol, ShortWin, before an auditor or the market does it for me*

By Todor Bonchev, founder of ShortWin

I’m building ShortWin, a Solana protocol that lets people lend and short memecoins, including tokens still on the pump.fun bonding curve.

ShortWin is pre-audit. An uninitialized version of its program is deployed on Solana devnet for testing, but no ShortWin market is available to users. This is a design essay, not an offer or investment advice. The numbers below come from specified examples in my own economic model. They can be wrong, and they are not forecasts of live returns.

The hardest bugs in a lending protocol aren’t necessarily in the code. They can be in the incentives. A program can follow its specification and still move money from the naive to the clever.

So I asked what every participant would do if they cared only about their own payoff. Reviewing those answers also exposed mistakes in how I described the mechanism. Those deserve correction before they become design decisions.

## The mechanism in 60 seconds

Lenders deposit tokens. Shorters post SOL margin, borrow those tokens and sell them through the supported venue. The sale proceeds stay locked with the margin. The borrower cannot withdraw them and disappear.

Rent goes 85% to lenders and 15% to the protocol. Its curve has a kink at 80% utilization, and new borrowing is capped at 90%. A position keeps the rent-curve coefficients it opened with. **Its actual APR still changes with utilization**, and its SOL rent follows the tokens’ value at the smoothed price. I previously described that as a locked rate. That was wrong.

Tier D, for bonding-curve tokens, has a 72-hour maximum term. Tier A has 30 days. Borrowers can close earlier. A deadline makes a position eligible for expiry; it does not guarantee that tokens or SOL arrive at that exact moment.

Liquidations require both live and smoothed evidence. If a buyback cannot be completed under the settlement rules, lenders can receive SOL instead of tokens. Lenders bear the remaining loss. There is no insurance fund or treasury backstop, and markets are isolated.

That is the board. Now the players.

## Lenders: rent comes with forgone upside

The uncomfortable part is that a lender can earn SOL and still do worse than simply holding the token.

In a normal repayment, the tokens come back with rent. If a pump outruns the collateral and the position settles in SOL, the lender can receive less than the missing tokens are worth.

Relative to holding, that resembles selling a call: collect income and surrender some upside in an extreme move. The whole position still has token exposure. It is closer to holding a token with a contingent upside cap than being short a standalone call. Repayment timing, shared pools and mixed token/SOL balances make the actual payoff more complicated. Existing share holders, including late depositors, can also share an accrued-rent shortfall and need additional SOL before withdrawing tokens.

In the model’s example markets, a 10× gap leaves lenders about 58–78% behind holding the lent tokens. That percentage uses the tokens’ **terminal value**. It is not a loss of 58–78% of the original loan notional or the whole lender pool.

Other tables use opening notional. In one illustrative “calm” regime, Tier C averaged approximately −13.6% of opening notional versus holding, with considerable sampling uncertainty. Losses against opening notional can exceed 100% because the missing tokens may appreciate far beyond their original value. That does not by itself mean a lender paid an additional 100% of original principal in SOL.

The model’s gap frequencies and tail assumptions are uncalibrated. Its roughly four-week Tier D break-even interval belongs to one particular distribution and full-term income assumption. It is not an observed market frequency or a universal safe lending threshold.

There can still be a rational reason to lend: you expect rent to compensate for those risks. But “my bag otherwise earns nothing” is insufficient. Holding preserves upside and liquidity. Lending is also not a guaranteed forward sale: ordinary repayment gives you tokens, not an agreed SOL exit price.

## Insiders, concentration and held prices

An insider can supply lending inventory, short from another wallet, own venue liquidity and trade the remaining supply. That combination matters more than any one transaction.

It does not follow that every insider deposit is a completed exit. The lender usually retains a token claim, and shorts may later buy the tokens back. To show an extraction, I need to consolidate the actor’s wallets and compare the final SOL and inventory with a direct sale.

Concentration disclosure would help users assess dependence on large wallets. It would not identify beneficial owners or prove who is an insider. One person can split across addresses.

Held price manipulation is a separate problem. A price kept high for whole seconds can look exactly like genuine demand to rules observing that venue. An honest observation cannot tell me the trader’s private motive.

The possible profit depends on capital, swap fees, other traders selling into the push and keeper competition. My recorded no-arbitrage examples show meaningful harm. They are conditional examples, not promises that every such trade is profitable on a live market.

Nor does a 2% execution bound cap every attack at 2% of an untouched price. The bound is relative to a reference. A held path can lift that reference too.

## Curve completion is a special risk

On a standard constant-product bonding curve, completing from 80% progress implies approximately a 2.46× price move under the model’s exact-completion assumptions.

The standard migration example also produces an approximately 45.66% boost-related increase, with the model spending that boost over roughly six minutes. That is conditional arithmetic, not a guaranteed future price path. Different migration seeds, synthetic completion, donations, other trades and venue upgrades can change it.

In the recorded exact-completion example opened at 50% progress, lenders finish about 34.7% behind holding at the terminal token price. Examples opened at 80–90% instead return the tokens through normal liquidation. A later-progress example can survive graduation altogether.

This is why “raise rent as progress rises” is not yet an answer. Earlier completion can imply a larger jump but require more capital; later completion may be easier but produce a smaller jump. Probability, severity and shorter demand all matter.

Existing controls include a conservative remaining-inventory guard, a gate against new opens during the configured boost window, and migration/SOL-settlement timing rules. They reduce particular exposures. They do not establish that lenders receive the right risk premium.

## Watching prices costs money

Liquidation uses a five-second EMA time constant and persistence across three updates. Settlement has a separate history and confirmation process.

The eight-bucket settlement design makes certain manipulations require five attacker-only seconds. That does **not** mean every attack or delay requires five seconds without an honest keeper. Contrary prints can widen uncertainty or veto another required condition. Sometimes that is manipulation; sometimes the market genuinely recovered.

Honest observations benefit everyone exposed to the market, but the current mechanism does not reward each observation. It relies on an explicitly funded keeper operation.

Recorded cost models show why that matters. With six-way batching and zero priority price, continuous observation costs about 0.072 SOL per market-day before other duties. Higher priority prices, fewer markets to batch and retries change the bill. These estimates use historical compute measurements, not a fresh demonstration that observations will land during congestion.

Coverage is targeted and budgeted. If the keeper cannot cover a market, its coverage pause stops new opens. The pause itself leaves exits enabled. It cannot remove the risk already taken or guarantee that every exit satisfies the route, price and execution conditions.

A paid observation market might help. Paying for every print could also reward redundant work, self-triggered stress or dishonest observations. Payment is not proof of truth.

## Keeper rewards need a funding story

Normal liquidation pays a keeper 30% of the penalty, with a top-up toward the settlement reward when borrower equity is available. It does not pay the full penalty share **plus** another full settlement reward.

That settlement reward defaults to 0.01 SOL plus 1% of the vault, capped at 0.25 SOL. The normal-liquidation top-up is limited by the borrower’s remaining equity.

Expiry already has a fixed component: 0.001 SOL plus 0.1% of the reference buyback cost, capped at 0.05 SOL and limited by the available remainder. What it lacks is a minimum matching the larger settlement reward.

For a 0.1 SOL reference buyback, the nominal expiry reward is 0.0011 SOL. One recorded modeled **liquidation** episode, including an assumed 0.001 SOL tip, costs roughly 0.00103 SOL at the chosen priority-price ceiling. That is not a measured expiry cost or a guarantee about congestion.

For CU-priced transactions, Solana priority fees depend on the requested compute limit and CU price; v1 instead uses an absolute priority fee in lamports. Failed attempts, tips and landing conditions matter too. The narrow modeled margin deserves investigation, but it does not prove that a 0.01 SOL expiry minimum is necessary.

That change would add 0.009 SOL: 9% of the minimum 0.1 SOL notional. Someone pays for a better incentive. A guaranteed reward also needs guaranteed funding; a payment limited to leftover equity is not one.

## SOL settlement reallocates risk; it does not remove it

The fallback distinguishes a genuine funding gap from a thin or unavailable route.

For a gap, lenders receive the vault less the keeper reward. For thin-liquidity or no-route settlement, their take is capped at the specified fair value: executable debt cost at the smoothed state, applicable charges and rent, subject to available funds. Excess returns to the borrower.

The cap matters because unrestricted collateral seizure could reward someone who helps make a route unusable. Premium income also vests rather than becoming an instant prize for a depositor arriving around settlement.

These controls do not make cash equivalent to returned tokens. If I expect more upside, tokens would normally preserve it better. A preference for SOL settlement needs another reason: a premium, a desire to exit, a different forecast or a combined LP/inventory position.

I also need to describe liquidity protection accurately. Raydium’s rules use locked-depth and locked-SOL conditions; they do not require every eligible pool to have at least half its whole LP supply locked. PumpSwap can have third-party withdrawable LP. ShortWin sizes against the burned depth, not a claim that the whole pool is permanently locked.

## Collusion and comparisons

Self-lending can rebate much of my own rent, depending on how much of the lender pool I own. Capturing both lender and keeper penalty shares requires controlling those roles too. A consolidated calculation must include the remaining protocol and venue costs.

The 25% wallet cap is an address limit. Four wallets can fill a market. Market-wide caps carry the actual exposure bound.

There are no points or volume rewards, which removes one wash-trading subsidy. It does not eliminate motives to manufacture signals, push utilization or manipulate prices.

Shortr makes different documented choices: open-ended rolled shorts and time-locked lender buckets. Its public docs also describe an insurance fund that compensates bad debt before lenders absorb residual losses. I have not independently verified its deployed behavior. Comparing only its residual token loss with ShortWin’s SOL repayment would omit that first-loss layer.

## What I’m changing, and what still needs a design

The change in this branch is wording and explanation. It introduces no new economic mechanism. I removed the separate toy model’s attack-profit and top-5% figures from this version because they have not been reproduced as evidence for these decisions.

My priorities are now:

1. **Completion risk:** measure it across curve states before choosing progress-based rent or caps. A cap reduces exposure; it does not pay lenders a premium.
2. **First-loss capital:** design a funded, capped, market-isolated buffer before promising protection. The protocol’s 15% rent share also has to support operations.
3. **Coverage:** specify who pays, what delivery is rewarded and how abuse is bounded before introducing observation rewards.
4. **Curve OI:** validate any tighter limit against attack economics, available position sizes, minimum notional and lost fee income. No arbitrary reduction is adopted here.
5. **Withdrawals:** compare a utilization fee with existing locks and vesting. Making exits more expensive can encourage earlier runs and hurt legitimate lenders.
6. **Rent:** correct the explanation. Existing positions already respond to utilization; another floating-rate mechanism is unnecessary.
7. **Expiry:** measure costs and incentives before choosing a stronger minimum. Preserve clear funding and borrower-cost limits.
8. **Concentration:** prepare accurate wallet-level disclosure using complete, comparable ownership data. It is useful context, not identity verification.
9. **Deferral:** reject unconditional forced settlement after a timer. Genuine contrary evidence must still count. Any terminal fallback needs its own safe allocation rule.

I still want people to challenge this design. But an honest challenge needs a precise benchmark, a reproducible example and a clear account of who funds the remedy.

An auditor can check whether the code follows the rules. I also need to establish why anyone should accept those rules—and say plainly where I have not done that yet.

### Sources and scope

- [ShortWin source and documentation](https://github.com/TodorBonchev/short.win): economic-model sections on rent, settlement, graduation, keeper costs and lender outcomes. Figures here are illustrative recorded model results, not calibrated return forecasts or a new audit.
- [Solana transaction fees](https://solana.com/docs/core/fees): base fees, requested-limit priority fees for CU-priced transactions, and absolute priority fees in v1.
- [Shortr’s public documentation](https://shortr.fun/docs): its stated bucket, rollover and insurance design; not independently verified on-chain for this article.
