FINDING | program: shortwin-keeper | function: resolve_entry | bug_class: unsupported-coin-creator-placeholder | group_key: shortwin-keeper | resolve_entry | unsupported-coin-creator-placeholder
location: services/keeper/src/venue.rs:657, especially :670-677; choice_of :704; extras_candidates :738-746.
title: The keeper omits valid coin creator extras.
seam: access×asymmetry, with an economic exit consequence when the venue requires the extra.
actor: An honest Emergency or Governance signer publishes a supported PumpSwap template; the keeper then omits it.
root_cause: The keeper resolves four placeholders but rejects PLACEHOLDER_COIN_CREATOR, which the on-chain setter accepts and the trade parser resolves.
path: Authorized set_venue_extras(PumpSwap, EXTRAS_IX_BUY, template) → current stores the valid template → extras_candidates → choice_of → resolve_entry returns None → keeper excludes current from its exit attempts.
proof: programs/shortwin/src/state/venue_extras.rs:60 defines PLACEHOLDER_COIN_CREATOR = 4. programs/shortwin/src/venues/mod.rs:297-333 accepts a SEED_PLACEHOLDER whose len is 4 and whose bytes are zero. programs/shortwin/src/instructions/set_venue_extras.rs:176-190 stores the checked set after the Signer/config authority checks at :101-117. programs/shortwin/src/venues/mod.rs:1266-1268 resolves this exact placeholder to pump_amm::cached(market, CACHED_COIN_CREATOR) for PumpSwap. Thus the stored creator is available without another RPC read. Take a PumpSwap market with cached creator C and one template entry: kind = ENTRY_TEMPLATE, key = an external program P, n_seeds = 1, seeds[0] = {kind: SEED_PLACEHOLDER, len: 4, bytes: [0; 32]}, writable = 0, with unused seeds zero. Choose P and C so the derived PDA is outside both denylists. The on-chain template resolver derives PDA(P, [C, canonical bump]). In the keeper, len = 4 reaches `_ => None` at services/keeper/src/venue.rs:676, and `?` at :677 discards the entry. `?` at :704 discards the whole set. If current has this entry and previous/stable are empty, extras_candidates returns exactly one choice, NONE, rather than current plus NONE. Out-of-scope caller context services/keeper/src/keeper.rs:2861-2862 and :2867-2880 confirms liquidate/expire iterate only this candidate list. Republishing the same template as previous or stable cannot help because all three use choice_of.
description: The keeper skips valid coin creator templates, so operators cannot use those templates to restore automatic exits after a venue change.
guards: The setter requires an authorized signer and checks template shape. The on-chain parser verifies the extras PDA, version, venue, canonical bump, signer status, ownership, denylists, and writable flags. Those guards accept this supported template and do not repair the keeper omission.
impact: The supported-template omission is unconditional for placeholder 4. If a venue needs that extra for buys, the keeper never tries the valid liquidation or expiry account list. A position with Q tokens owed and V lamports remains open after the failed automatic attempt; this report does not assert a specific loss amount.
limitations: This is a proven integration correctness defect. No currently deployed venue requirement for this extra was established. A manually built transaction can select and pass the valid template. No build, test, or runtime reproduction was performed.
fix: Resolve PLACEHOLDER_COIN_CREATOR from the PumpSwap cache, as the on-chain parser does.
minimal_diff: Unapplied and uncompiled.
```diff
--- a/services/keeper/src/venue.rs
+++ b/services/keeper/src/venue.rs
@@ -672,7 +672,14 @@
                             PLACEHOLDER_POOL => pool_placeholder(kind, market),
                             PLACEHOLDER_TRADER => Some(market.trader),
                             PLACEHOLDER_BONDING_CURVE => Some(market.bonding_curve),
-                            // The coin creator is not resolvable from the market cache.
+                            shortwin::state::PLACEHOLDER_COIN_CREATOR
+                                if kind == VenueKind::PumpSwap =>
+                            {
+                                Some(pump_amm::cached(
+                                    market,
+                                    pump_swap_cache::CACHED_COIN_CREATOR,
+                                ))
+                            }
                             _ => None,
                         }?;
                         owned.push(key.to_bytes().to_vec());
```

LEAD | program: shortwin-keeper | function: decode_zc | bug_class: hardening-version-unchecked | group_key: shortwin-keeper | decode_zc | hardening-version-unchecked
location: services/keeper/src/view.rs:48-55; load_with_positions :252-388.
title: The keeper accepts layouts without a version check.
seam: access×asymmetry across the on-chain account authority and the keeper's interpretation.
code_smells: The decoder checks the owner, discriminator, and minimum length. The caller does not check the GlobalConfig, Market, or VenueExtras version.
proof: decode_zc accepts any shortwin-owned account whose discriminator matches T and whose data contains size_of::<T>() bytes after offset 8. It returns pod_read_unaligned without inspecting a version field. load_with_positions accepts GlobalConfig through this helper, then Market and MarketVault, and finally VenueExtras. No typed version guard follows those loads. In contrast, programs/shortwin/src/instructions/set_coverage_pause.rs:69 and :84 checks CONFIG_VERSION and MARKET_VERSION; programs/shortwin/src/venues/mod.rs:189-192 checks VENUE_EXTRAS_VERSION before the on-chain consumer uses extras. A future account with the same discriminator and sufficient bytes, but another version, therefore enters keeper decision state even when the on-chain consumer rejects it.
description: A future layout change can make the keeper use unsupported state because its decoder omits the version checks that on-chain consumers require.
impact: The keeper can derive incorrect decisions or account lists during a future version transition. The current frozen state provides no outsider path to rewrite a shortwin-owned version.
limitations: This is a hardening lead, not a demonstrated present exploit. Transaction simulation and on-chain checks still restrict execution. The generic decoder alone cannot know each type's expected version.
fix: Add typed version checks at each relevant load before the keeper uses the decoded account.
