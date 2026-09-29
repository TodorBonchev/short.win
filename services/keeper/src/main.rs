//! `shortwin-keeper`: the keeper against a JSON-RPC cluster (ARCHITECTURE §7.1).
//!
//! ```text
//! shortwin-keeper --rpc <url> --keypair <file> [--config keeper.toml] [--tick-ms 400]
//!                 [--metrics 127.0.0.1:9464] [--discovery-rpc <url>] [--block-engine <url>]
//!                 [--once]
//! ```
//!
//! `--block-engine` (or `graduation.block_engine`) sends the straddle bundles (duty 6, ARCH §5.4)
//! to a Jito block engine, e.g. `https://mainnet.block-engine.jito.wtf`; without one they are not
//! sent (the keeper alerts and calls `sol_settle` instead).
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
use shortwin_keeper::{Keeper, KeeperConfig};
use shortwin_services_common::rpc::RpcChain;
use solana_keypair::read_keypair_file;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Args {
    rpc: String,
    keypair: String,
    config: Option<String>,
    tick_ms: u64,
    metrics: Option<String>,
    discovery_rpc: Option<String>,
    block_engine: Option<String>,
    once: bool,
}

fn usage() -> &'static str {
    "usage: shortwin-keeper --rpc <url> --keypair <file> [--config keeper.toml] [--tick-ms 400] \
     [--metrics 127.0.0.1:9464] [--discovery-rpc <url>] [--block-engine <url>] [--once]"
}

fn parse_args() -> anyhow::Result<Args> {
    let mut rpc = None;
    let mut keypair = None;
    let mut config = None;
    let mut tick_ms = 400;
    let mut metrics = None;
    let mut discovery_rpc = None;
    let mut block_engine = None;
    let mut once = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .with_context(|| format!("{a} needs a value\n{}", usage()))
        };
        match a.as_str() {
            "--rpc" => rpc = Some(val()?),
            "--keypair" => keypair = Some(val()?),
            "--config" => config = Some(val()?),
            "--tick-ms" => tick_ms = val()?.parse().context("--tick-ms")?,
            "--metrics" => metrics = Some(val()?),
            "--discovery-rpc" => discovery_rpc = Some(val()?),
            "--block-engine" => block_engine = Some(val()?),
            "--once" => once = true,
            "-h" | "--help" => bail!("{}", usage()),
            other => bail!("unknown argument {other}\n{}", usage()),
        }
    }
    Ok(Args {
        rpc: rpc.with_context(|| format!("--rpc is required\n{}", usage()))?,
        keypair: keypair.with_context(|| format!("--keypair is required\n{}", usage()))?,
        config,
        tick_ms,
        metrics,
        discovery_rpc,
        block_engine,
        once,
    })
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
    let cfg = match &args.config {
        Some(p) => KeeperConfig::from_toml(
            &std::fs::read_to_string(p).with_context(|| format!("reading {p}"))?,
        )?,
        None => KeeperConfig::default(),
    };
    let payer = read_keypair_file(&args.keypair)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", args.keypair))?;
    let mut chain = RpcChain::new(args.rpc.clone());
    let engine = args
        .block_engine
        .clone()
        .or_else(|| cfg.graduation.block_engine().map(str::to_string));
    if let Some(url) = &engine {
        tracing::info!(%url, "Jito block engine (straddle bundles)");
        chain = chain.with_block_engine(url);
    }
    let discovery = args
        .discovery_rpc
        .clone()
        .or_else(|| cfg.pool_select.discovery_rpc().map(str::to_string));
    let mut keeper = Keeper::new(chain, payer, cfg)?;
    if let Some(url) = discovery {
        tracing::info!(%url, "pool discovery endpoint");
        keeper = keeper.with_discovery_chain(RpcChain::new(url));
    }
    tracing::info!(payer = %keeper.payer(), rpc = %args.rpc, "keeper starting");

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
