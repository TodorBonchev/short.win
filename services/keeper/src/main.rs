//! `shortwin-keeper`: the keeper against a JSON-RPC cluster (ARCHITECTURE §7.1).
//!
//! ```text
//! shortwin-keeper --rpc <url> --keypair <file> [--config keeper.toml] [--tick-ms 400]
//!                 [--metrics 127.0.0.1:9464] [--discovery-rpc <url>]
//!                 [--coverage-keypair <file>] [--once]
//! shortwin-keeper --rpc <url> --observe [--tick-ms 30000]
//!                 [--metrics 127.0.0.1:9464] [--once]
//! ```
//!
//! `--coverage-keypair` (or `coverage_pause.authority_keypair`) loads the `coverage_authority` key
//! of the designated pauser (O17; a separate key from the fee payer, which can only pause and
//! unpause opens per market); it is required with `coverage_pause.enabled = true`, and only one
//! running instance should be enabled.
//!
//! `--block-engine` is removed with the straddle bundle it served (pump-final §7, owner D2: a curve
//! position past `rT` exits by pump v3's synthetic migration in its own `liquidate` / `expire`);
//! naming it is refused.
//!
//! `--discovery-rpc` (or `pool_select.discovery_rpc` in the configuration) sends the Raydium
//! pool discovery's `getProgramAccounts` calls to another endpoint (duty 12).
//!
//! One tick per `--tick-ms` (a slot by default); alerts and errors go to the log
//! (`RUST_LOG`, default `info`); `--metrics` serves the Prometheus counters on `/metrics`.
//! The keypair is a hot fee payer with no protocol authority (ARCH §7.1 "Keys and ops").

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use shortwin::constants::{CONFIG_VERSION, VENUE_GUARD_VERSION};
use shortwin::state::{GlobalConfig, OracleCache, VenueGuard};
use shortwin_keeper::{Keeper, KeeperConfig, pda, view};
use shortwin_services_common::Chain;
use shortwin_services_common::rpc::RpcChain;
use solana_keypair::read_keypair_file;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Args {
    rpc: String,
    keypair: Option<String>,
    config: Option<String>,
    tick_ms: u64,
    metrics: Option<String>,
    discovery_rpc: Option<String>,
    coverage_keypair: Option<String>,
    once: bool,
    observe: bool,
}

fn usage() -> &'static str {
    "usage: shortwin-keeper --rpc <url> --keypair <file> [--config keeper.toml] [--tick-ms 400] \
     [--metrics 127.0.0.1:9464] [--discovery-rpc <url>] [--coverage-keypair <file>] [--once]\n\
     or: shortwin-keeper --rpc <url> --observe [--tick-ms 30000] \
     [--metrics 127.0.0.1:9464] [--once]"
}

fn parse_args_from(mut it: impl Iterator<Item = String>) -> anyhow::Result<Args> {
    let mut rpc = None;
    let mut keypair = None;
    let mut config = None;
    let mut tick_ms = 400;
    let mut metrics = None;
    let mut discovery_rpc = None;
    let mut coverage_keypair = None;
    let mut once = false;
    let mut observe = false;
    let mut tick_ms_set = false;
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .with_context(|| format!("{a} needs a value\n{}", usage()))
        };
        match a.as_str() {
            "--rpc" => rpc = Some(val()?),
            "--keypair" => keypair = Some(val()?),
            "--config" => config = Some(val()?),
            "--tick-ms" => {
                tick_ms = val()?.parse().context("--tick-ms")?;
                tick_ms_set = true;
            }
            "--metrics" => metrics = Some(val()?),
            "--discovery-rpc" => discovery_rpc = Some(val()?),
            "--block-engine" => bail!(
                "--block-engine: {}",
                shortwin_keeper::config::REMOVED_GRADUATION_MESSAGE
            ),
            "--coverage-keypair" => coverage_keypair = Some(val()?),
            "--once" => once = true,
            "--observe" => observe = true,
            "-h" | "--help" => bail!("{}", usage()),
            other => bail!("unknown argument {other}\n{}", usage()),
        }
    }
    if observe {
        if keypair.is_some()
            || coverage_keypair.is_some()
            || config.is_some()
            || discovery_rpc.is_some()
        {
            bail!("--observe accepts no keypair, configuration, or discovery endpoint");
        }
        if !tick_ms_set {
            tick_ms = 30_000;
        }
    } else if keypair.is_none() {
        bail!("--keypair is required unless --observe is set\n{}", usage());
    }
    Ok(Args {
        rpc: rpc.with_context(|| format!("--rpc is required\n{}", usage()))?,
        keypair,
        config,
        tick_ms,
        metrics,
        discovery_rpc,
        coverage_keypair,
        once,
        observe,
    })
}

fn parse_args() -> anyhow::Result<Args> {
    parse_args_from(std::env::args().skip(1))
}

/// Startup context must not expose provider credentials in userinfo, path, query or fragment.
/// Preserve only an HTTP(S) host and optional port; malformed inputs fail closed.
fn rpc_endpoint_for_log(endpoint: &str) -> String {
    const REDACTED: &str = "REDACTED";
    let Some((scheme, rest)) = endpoint.split_once("://") else {
        return REDACTED.to_string();
    };
    if !matches!(scheme, "http" | "https") {
        return REDACTED.to_string();
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let valid = if let Some(ipv6) = host.strip_prefix('[') {
        ipv6.split_once(']').is_some_and(|(address, port)| {
            address.parse::<std::net::Ipv6Addr>().is_ok()
                && (port.is_empty()
                    || port
                        .strip_prefix(':')
                        .is_some_and(|value| value.parse::<u16>().is_ok()))
        })
    } else {
        let (name, port) = host.rsplit_once(':').unwrap_or((host, ""));
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
            && (port.is_empty() && !host.ends_with(':') || port.parse::<u16>().is_ok())
    };
    if !valid {
        return REDACTED.to_string();
    }
    format!("{scheme}://{host}")
}

fn log_observer_startup(rpc: &str) {
    tracing::info!(rpc = %rpc_endpoint_for_log(rpc), "keeper observe starting without a keypair");
}

fn log_keeper_startup(payer: &solana_pubkey::Pubkey, rpc: &str) {
    tracing::info!(%payer, rpc = %rpc_endpoint_for_log(rpc), "keeper starting");
}

fn log_discovery_endpoint(url: &str) {
    tracing::info!(url = %rpc_endpoint_for_log(url), "pool discovery endpoint");
}

/// A read-only readiness snapshot. This path never creates a signer or transaction.
async fn observe_tick(chain: &RpcChain) -> anyhow::Result<String> {
    let clock = chain.clock().await?;
    let keys = [pda::config(), pda::venue_guard(), pda::oracle_cache()];
    let accounts = chain.accounts(&keys).await?;
    let config = accounts[0]
        .as_ref()
        .and_then(view::decode_zc::<GlobalConfig>);
    let guard = accounts[1].as_ref().and_then(view::decode_zc::<VenueGuard>);
    let oracle = accounts[2]
        .as_ref()
        .and_then(view::decode_borsh::<OracleCache>);
    if accounts[0].is_some() && config.is_none() {
        anyhow::bail!("GlobalConfig exists but cannot be decoded");
    }
    if accounts[1].is_some() && guard.is_none() {
        anyhow::bail!("VenueGuard exists but cannot be decoded");
    }
    if accounts[2].is_some() && oracle.is_none() {
        anyhow::bail!("OracleCache exists but cannot be decoded");
    }
    if let Some(c) = &config {
        anyhow::ensure!(
            c.version == CONFIG_VERSION,
            "GlobalConfig version {} is unsupported",
            c.version
        );
    }
    if let Some(g) = &guard {
        anyhow::ensure!(
            g.version == VENUE_GUARD_VERSION,
            "VenueGuard version {} is unsupported",
            g.version
        );
    }
    // Before initialization there can be no markets; avoid expensive program-account scans.
    let (markets, positions) = if config.is_some() {
        tokio::try_join!(view::discover_markets(chain), view::open_positions(chain))?
    } else {
        (Vec::new(), std::collections::HashMap::new())
    };
    let open_positions: usize = positions.values().map(Vec::len).sum();
    tracing::info!(
        slot = clock.slot,
        config = config.is_some(),
        venue_guard = guard.is_some(),
        oracle_cache = oracle.is_some(),
        markets = markets.len(),
        open_positions,
        "keeper observe (read-only; no coverage transactions)"
    );
    Ok(format!(
        "# HELP shortwin_keeper_observe_mode Read-only mode; no keeper transactions are sent.\n\
         # TYPE shortwin_keeper_observe_mode gauge\nshortwin_keeper_observe_mode 1\n\
         # TYPE shortwin_keeper_observe_slot gauge\nshortwin_keeper_observe_slot {}\n\
         # TYPE shortwin_keeper_config_initialized gauge\nshortwin_keeper_config_initialized {}\n\
         # TYPE shortwin_keeper_venue_guard_initialized gauge\nshortwin_keeper_venue_guard_initialized {}\n\
         # TYPE shortwin_keeper_oracle_cache_initialized gauge\nshortwin_keeper_oracle_cache_initialized {}\n\
         # TYPE shortwin_keeper_markets gauge\nshortwin_keeper_markets {}\n\
         # TYPE shortwin_keeper_open_positions gauge\nshortwin_keeper_open_positions {}\n",
        clock.slot,
        u8::from(config.is_some()),
        u8::from(guard.is_some()),
        u8::from(oracle.is_some()),
        markets.len(),
        open_positions,
    ))
}

async fn run_observer(args: &Args) -> anyhow::Result<()> {
    let chain = RpcChain::new(args.rpc.clone());
    let text = Arc::new(Mutex::new(String::new()));
    if let Some(addr) = args.metrics.clone() {
        let t = text.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_metrics(addr, t).await {
                tracing::error!("metrics endpoint: {e:#}");
            }
        });
    }
    log_observer_startup(&args.rpc);
    let mut interval = tokio::time::interval(Duration::from_millis(args.tick_ms.max(50)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
        match observe_tick(&chain).await {
            Ok(metrics) => {
                if let Ok(mut t) = text.lock() {
                    *t = metrics;
                }
            }
            Err(e) if args.once => return Err(e.context("observe tick")),
            Err(e) => tracing::error!("observe tick: {e:#}"),
        }
        if args.once {
            return Ok(());
        }
    }
}

/// Serves `text` on every request (a minimal `/metrics` endpoint).
async fn serve_metrics(addr: String, text: Arc<Mutex<String>>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "metrics on /metrics");
    loop {
        let (mut sock, _) = listener.accept().await?;
        let body = text.lock().map(|t| t.clone()).unwrap_or_default();
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/plain; version=0.0.4\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes()).await;
        });
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let args = parse_args()?;
    if args.observe {
        return run_observer(&args).await;
    }
    let cfg = match &args.config {
        Some(p) => KeeperConfig::from_toml(
            &std::fs::read_to_string(p).with_context(|| format!("reading {p}"))?,
        )?,
        None => KeeperConfig::default(),
    };
    let keypair = args.keypair.as_deref().context("--keypair is required")?;
    let payer =
        read_keypair_file(keypair).map_err(|e| anyhow::anyhow!("reading {keypair}: {e}"))?;
    let chain = RpcChain::new(args.rpc.clone());
    let discovery = args
        .discovery_rpc
        .clone()
        .or_else(|| cfg.pool_select.discovery_rpc().map(str::to_string));
    let coverage_key = args
        .coverage_keypair
        .clone()
        .or_else(|| cfg.coverage_pause.authority_keypair().map(str::to_string));
    if cfg.coverage_pause.enabled && coverage_key.is_none() {
        bail!(
            "coverage_pause.enabled needs the coverage_authority keypair \
             (--coverage-keypair or coverage_pause.authority_keypair)"
        );
    }
    let coverage = match &coverage_key {
        Some(p) => Some(read_keypair_file(p).map_err(|e| anyhow::anyhow!("reading {p}: {e}"))?),
        None => None,
    };
    if let Some(k) = &coverage
        && solana_signer::Signer::pubkey(k) == solana_signer::Signer::pubkey(&payer)
    {
        bail!("the coverage_authority key must not be the fee payer");
    }
    let enabled = cfg.coverage_pause.enabled;
    let mut keeper = Keeper::new(chain, payer, cfg)?;
    if let Some(k) = coverage {
        keeper = keeper.with_coverage_authority(k);
        tracing::info!(
            coverage_authority = ?keeper.coverage_authority(),
            designated_pauser = enabled,
            "coverage_authority key loaded (O17)"
        );
    }
    if let Some(url) = discovery {
        log_discovery_endpoint(&url);
        keeper = keeper.with_discovery_chain(RpcChain::new(url));
    }
    log_keeper_startup(&keeper.payer(), &args.rpc);

    let text = Arc::new(Mutex::new(String::new()));
    if let Some(addr) = args.metrics.clone() {
        let t = text.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_metrics(addr, t).await {
                tracing::error!("metrics endpoint: {e:#}");
            }
        });
    }

    let mut interval = tokio::time::interval(Duration::from_millis(args.tick_ms.max(50)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("stopping");
                return Ok(());
            }
        }
        match keeper.tick().await {
            Ok(r) => {
                for e in &r.errors {
                    tracing::warn!("{e}");
                }
                for x in &r.exits {
                    tracing::info!(
                        market = %x.market,
                        position = %x.position,
                        kind = ?x.kind,
                        plan = ?x.plan,
                        landed = x.result.landed().is_some(),
                        "exit"
                    );
                }
                if !r.cranked.is_empty() {
                    tracing::debug!(n = r.cranked.len(), batches = r.batches.len(), "cranked");
                }
            }
            Err(e) => tracing::error!("tick: {e:#}"),
        }
        if let Ok(mut t) = text.lock() {
            *t = keeper.metrics.render();
        }
        if args.once {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_args_from;
    use std::sync::{Arc, Mutex};

    fn args(items: &[&str]) -> anyhow::Result<super::Args> {
        parse_args_from(items.iter().map(|s| (*s).to_string()))
    }

    #[derive(Clone)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn startup_logs_keep_endpoint_context_without_provider_credentials() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = LogBuffer(bytes.clone());
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || writer.clone())
            .finish();
        let endpoint = "https://provider-user:provider-password@rpc.example.test:8443/secret-path-token?api-key=secret-api-key&session=x#secret-fragment";
        let payer = solana_pubkey::Pubkey::new_from_array([7; 32]);
        tracing::subscriber::with_default(subscriber, || {
            super::log_observer_startup(endpoint);
            super::log_keeper_startup(&payer, endpoint);
            super::log_discovery_endpoint(endpoint);
        });
        let rendered = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        for event in [
            "keeper observe starting without a keypair",
            "keeper starting",
            "pool discovery endpoint",
        ] {
            assert!(rendered.contains(event), "missing startup event {event}");
        }
        assert_eq!(rendered.matches("https://rpc.example.test:8443").count(), 3);
        assert!(rendered.contains(&payer.to_string()));
        for secret in [
            "provider-user",
            "provider-password",
            "secret-path-token",
            "api-key",
            "secret-api-key",
            "session=x",
            "secret-fragment",
        ] {
            assert!(!rendered.contains(secret), "startup log exposed {secret}");
        }
    }

    #[test]
    fn endpoint_context_handles_root_ipv6_and_invalid_urls_without_echoing_input() {
        for (endpoint, shown) in [
            (
                "https://mainnet.example.test/",
                "https://mainnet.example.test",
            ),
            ("http://127.0.0.1:18899/?token=s", "http://127.0.0.1:18899"),
            ("http://[::1]:18899/secret", "http://[::1]:18899"),
            (
                "https://user:password@[2001:db8::1]/?api-key=x",
                "https://[2001:db8::1]",
            ),
            ("private-api-key", "REDACTED"),
            ("https:///secret-path-token", "REDACTED"),
            ("https://host:not-a-port/secret", "REDACTED"),
            ("https://host:/secret", "REDACTED"),
            ("https://host\nsecret/", "REDACTED"),
            ("file://secret-path-token", "REDACTED"),
        ] {
            assert_eq!(super::rpc_endpoint_for_log(endpoint), shown);
        }
    }

    #[test]
    fn observe_requires_no_signer_and_defaults_to_thirty_second_poll() {
        let parsed = args(&["--rpc", "https://api.devnet.solana.com", "--observe"]).unwrap();
        assert!(parsed.observe);
        assert!(parsed.keypair.is_none());
        assert_eq!(parsed.tick_ms, 30_000);
    }

    #[test]
    fn observe_rejects_signing_inputs_and_active_mode_requires_keypair() {
        for extra in [
            ["--keypair", "payer.json"],
            ["--coverage-keypair", "pauser.json"],
            ["--config", "keeper.toml"],
            ["--discovery-rpc", "https://example.test"],
        ] {
            assert!(args(&["--rpc", "http://localhost", "--observe", extra[0], extra[1]]).is_err());
        }
        assert!(args(&["--rpc", "http://localhost"]).is_err());
        let active = args(&["--rpc", "http://localhost", "--keypair", "payer.json"]).unwrap();
        assert!(!active.observe);
        assert_eq!(active.keypair.as_deref(), Some("payer.json"));
    }

    #[tokio::test]
    async fn observe_once_returns_rpc_failure() {
        let parsed = args(&["--rpc", "http://127.0.0.1:0", "--observe", "--once"]).unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            super::run_observer(&parsed),
        )
        .await
        .expect("local invalid RPC should fail promptly");
        assert!(result.is_err());
    }
}
