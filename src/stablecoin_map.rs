//! Stablecoin map evidence-package data builder.
//!
//! This is the Rust replacement for the article-local Python data builders. It
//! keeps reproducible CSV generation in the repo and leaves publication graphics
//! to downstream figure tooling.

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

const DEFILLAMA_URL: &str = "https://stablecoins.llama.fi/stablecoins?includePrices=true";
const ARTEMIS_METRIC: &str = "ARTEMIS_STABLECOIN_TRANSFER_VOLUME";
const ARTEMIS_URL: &str =
    "https://data-svc.artemisxyz.com/data/api/ARTEMIS_STABLECOIN_TRANSFER_VOLUME";

const REPRESENTATIVE_SYMBOLS: &[&str] = &[
    "USDT", "USDC", "USDS", "USD1", "DAI", "USDe", "PYUSD", "RLUSD", "USDTB", "TUSD", "FDUSD",
    "FRAX", "EURC", "EURI", "EURE", "EURS", "EURA", "EUROe", "XSGD", "GYEN", "BRLA", "CADC",
    "AUDD", "COPm",
];

#[derive(Debug, Clone)]
pub struct PackageOptions {
    pub output_dir: PathBuf,
    pub dependency_summary: PathBuf,
    pub liquidity_pairs: PathBuf,
    pub artemis_start: String,
    pub artemis_end: String,
    pub skip_network: bool,
}

pub async fn run(options: PackageOptions) -> Result<()> {
    fs::create_dir_all(&options.output_dir)
        .with_context(|| format!("create {}", options.output_dir.display()))?;

    let local_rows = write_dependency_summary(&options.dependency_summary, &options.output_dir)
        .with_context(|| {
            format!(
                "build stablecoin_dependency_summary.csv from {}",
                options.dependency_summary.display()
            )
        })?;
    let edge_rows = write_dependency_edges(&options.liquidity_pairs, &options.output_dir)
        .with_context(|| {
            format!(
                "build stablecoin_dependency_edges.csv from {}",
                options.liquidity_pairs.display()
            )
        })?;

    if options.skip_network {
        eprintln!(
            "wrote local dependency package: {} summary rows, {} edge rows",
            local_rows, edge_rows
        );
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .user_agent("stablecoin-audit/0.1")
        .build()?;
    let inventory = build_inventory(&client, &options.output_dir).await?;
    write_inventory(&inventory, &options.output_dir)?;
    let transfer_rows = build_transfer_volume(
        &client,
        &inventory,
        &options.artemis_start,
        &options.artemis_end,
    )
    .await?;
    write_transfer_volume(&transfer_rows, &options.output_dir)?;
    eprintln!(
        "wrote stablecoin map package: {} inventory rows, {} transfer rows, {} dependency rows, {} edge rows",
        inventory.len(),
        transfer_rows.len(),
        local_rows,
        edge_rows
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
struct DefiLlamaResponse {
    #[serde(rename = "peggedAssets")]
    pegged_assets: Vec<DefiLlamaAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct DefiLlamaAsset {
    symbol: Option<String>,
    name: Option<String>,
    #[serde(rename = "pegType")]
    peg_type: Option<String>,
    #[serde(rename = "pegMechanism")]
    peg_mechanism: Option<String>,
    circulating: Option<HashMap<String, Value>>,
    price: Option<f64>,
    chains: Option<Vec<String>>,
    #[serde(rename = "chainCirculating")]
    chain_circulating: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InventoryRow {
    symbol: String,
    name: String,
    peg_currency: String,
    country_or_currency_zone: String,
    issuer_or_protocol: String,
    market_cap_or_circulating_supply: String,
    supply_source: String,
    chains_or_deployments: String,
    contract_addresses_if_available: String,
    observed_dex_liquidity_if_available: String,
    source_url_or_artifact: String,
    source_timestamp: String,
    confidence_grade: String,
    notes: String,
}

async fn build_inventory(client: &reqwest::Client, output_dir: &Path) -> Result<Vec<InventoryRow>> {
    let payload = client
        .get(DEFILLAMA_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<DefiLlamaResponse>()
        .await?;
    build_inventory_rows(
        payload,
        output_dir,
        Path::new("configs/tokens"),
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    )
}

fn build_inventory_rows(
    payload: DefiLlamaResponse,
    output_dir: &Path,
    config_dir: &Path,
    fetched_at: String,
) -> Result<Vec<InventoryRow>> {
    let mut by_symbol: HashMap<String, DefiLlamaAsset> = HashMap::new();
    for asset in payload.pegged_assets {
        let Some(symbol) = asset.symbol.clone() else {
            continue;
        };
        let replace = by_symbol
            .get(&symbol)
            .map(|existing| usd_value(&asset) > usd_value(existing))
            .unwrap_or(true);
        if replace {
            by_symbol.insert(symbol, asset);
        }
    }

    let addresses = load_config_addresses(config_dir)?;
    let liquidity = load_observed_liquidity(output_dir)?;
    let mut rows = Vec::new();

    for symbol in REPRESENTATIVE_SYMBOLS {
        let Some(asset) = by_symbol.get(*symbol) else {
            continue;
        };
        let peg = peg_currency(asset.peg_type.as_deref().unwrap_or_default());
        let country_zone = country_zone(&peg);
        let native = native_circulating(asset);
        let market_value_usd = usd_value(asset);
        let observed = liquidity.get(*symbol).copied();
        let confidence = if matches!(*symbol, "USDC" | "EURC" | "XSGD") && observed.is_some() {
            "high"
        } else if market_value_usd < 100_000.0 {
            "low"
        } else {
            "medium"
        };

        let mut notes = vec![
            "Representative footprint row; not a complete stablecoin universe.".to_string(),
            format!(
                "Peg mechanism reported by DefiLlama: {}.",
                asset.peg_mechanism.as_deref().unwrap_or("not reported")
            ),
        ];
        if addresses.contains_key(*symbol) {
            notes.push("Contract addresses are from project token configs.".to_string());
        } else {
            notes.push("Contract addresses not compiled in this inventory.".to_string());
        }
        if observed.is_none() {
            notes.push(
                "Observed DEX liquidity not available in current project artifacts.".to_string(),
            );
        }

        rows.push(InventoryRow {
            symbol: (*symbol).to_string(),
            name: asset.name.clone().unwrap_or_default(),
            peg_currency: peg.clone(),
            country_or_currency_zone: country_zone.to_string(),
            issuer_or_protocol: issuer(symbol, asset.name.as_deref().unwrap_or_default()).to_string(),
            market_cap_or_circulating_supply: format!(
                "{} {} circulating; approx ${} at DefiLlama price",
                comma0(native),
                peg,
                comma0(market_value_usd)
            ),
            supply_source: "DefiLlama stablecoins API".to_string(),
            chains_or_deployments: format_chains(asset, 12),
            contract_addresses_if_available: addresses.get(*symbol).cloned().unwrap_or_default().join("; "),
            observed_dex_liquidity_if_available: observed
                .map(|v| format!("${} observed DEX TVL in project DexScreener snapshot", comma0(v)))
                .unwrap_or_default(),
            source_url_or_artifact: format!(
                "{DEFILLAMA_URL}; data/benchmarks/stablecoin_dependency_summary.csv where DEX liquidity is shown; configs/tokens/*.yml where contract addresses are shown"
            ),
            source_timestamp: fetched_at.clone(),
            confidence_grade: confidence.to_string(),
            notes: notes.join(" "),
        });
    }
    Ok(rows)
}

fn write_inventory(rows: &[InventoryRow], output_dir: &Path) -> Result<()> {
    let mut writer = csv::Writer::from_path(output_dir.join("global_stablecoin_inventory_v1.csv"))?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct TransferVolumeRow {
    token: String,
    peg_currency: String,
    token_category: String,
    source: String,
    query_window_start: String,
    query_window_end: String,
    extraction_timestamp: String,
    total_30d_transfer_volume: String,
    non_null_days: usize,
    missing_days: isize,
    notes: String,
}

async fn build_transfer_volume(
    client: &reqwest::Client,
    inventory: &[InventoryRow],
    start: &str,
    end: &str,
) -> Result<Vec<TransferVolumeRow>> {
    let symbols: Vec<String> = inventory.iter().map(|row| row.symbol.clone()).collect();
    let lower = symbols
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("{ARTEMIS_URL}?symbols={lower}&startDate={start}&endDate={end}");
    let payload = client
        .get(ARTEMIS_URL)
        .query(&[
            ("symbols", lower.as_str()),
            ("startDate", start),
            ("endDate", end),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    Ok(build_transfer_volume_rows(
        payload,
        inventory,
        start,
        end,
        source,
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    ))
}

fn build_transfer_volume_rows(
    payload: Value,
    inventory: &[InventoryRow],
    start: &str,
    end: &str,
    source: String,
    extracted_at: String,
) -> Vec<TransferVolumeRow> {
    let mut rows = Vec::new();

    for item in inventory {
        let series = payload
            .pointer(&format!(
                "/data/symbols/{}/{}",
                item.symbol.to_lowercase(),
                ARTEMIS_METRIC
            ))
            .cloned();
        let values = series
            .as_ref()
            .and_then(Value::as_array)
            .map(|points| {
                points
                    .iter()
                    .filter_map(|point| point.get("val").and_then(Value::as_f64))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let unavailable = series.as_ref().and_then(Value::as_array).is_none();
        let total: f64 = values.iter().sum();
        let non_null_days = values.len();
        let mut notes = "Artemis adjusted stablecoin transfer-volume metric. Supported-token chart only; not complete global activity.".to_string();
        if unavailable {
            notes.push_str(&format!(
                " Metric unavailable response: {}",
                series.unwrap_or(Value::Null)
            ));
        }
        rows.push(TransferVolumeRow {
            token: item.symbol.clone(),
            peg_currency: item.peg_currency.clone(),
            token_category: token_category(&item.symbol).to_string(),
            source: source.clone(),
            query_window_start: start.to_string(),
            query_window_end: end.to_string(),
            extraction_timestamp: extracted_at.clone(),
            total_30d_transfer_volume: if unavailable {
                String::new()
            } else {
                format!("{total:.6}")
            },
            non_null_days,
            missing_days: 30 - non_null_days as isize,
            notes,
        });
    }
    rows
}

fn write_transfer_volume(rows: &[TransferVolumeRow], output_dir: &Path) -> Result<()> {
    let mut writer = csv::Writer::from_path(
        output_dir.join("stablecoin_transfer_volume_selected_rails_v1.csv"),
    )?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct PairDependenceInput {
    asset: String,
    chain: String,
    snapshot_utc: String,
    total_liquidity_usd: String,
    usdc_liquidity_usd: String,
    usdc_share: String,
    usd_stable_share: String,
    eur_stable_share: String,
    weth_share: String,
    other_share: String,
    top_pair: String,
    top_pair_liquidity_usd: String,
    top_pair_share: String,
    pool_count: String,
    notes: Option<String>,
}

#[derive(Debug, Serialize)]
struct DependencySummaryRow {
    asset: String,
    chain: String,
    deployment_id: String,
    snapshot_ts: String,
    total_observed_dex_liquidity_usd: String,
    usdc_pair_liquidity_usd: String,
    usdc_pair_share: String,
    usd_stable_pair_share: String,
    local_stable_pair_share: String,
    eur_stable_pair_share: String,
    weth_pair_share: String,
    other_pair_share: String,
    top_counterpart: String,
    top_pair_tvl_usd: String,
    top_pair_share: String,
    n_pools: String,
    n_counterpart_classes: String,
    caveat: String,
    source_file: String,
}

fn write_dependency_summary(input: &Path, output_dir: &Path) -> Result<usize> {
    let mut reader = csv::Reader::from_path(input)?;
    let mut writer = csv::Writer::from_path(output_dir.join("stablecoin_dependency_summary.csv"))?;
    let mut count = 0;
    for row in reader.deserialize::<PairDependenceInput>() {
        let row = row?;
        let out = DependencySummaryRow {
            deployment_id: format!("{}_{}", row.asset.to_lowercase(), row.chain.to_lowercase()),
            top_counterpart: top_counterpart(&row.top_pair, &row.asset),
            n_counterpart_classes: count_counterpart_classes(&row),
            caveat: row.notes.unwrap_or_else(|| {
                "DexScreener coverage is not complete market coverage; snapshot is descriptive."
                    .to_string()
            }),
            source_file: input.display().to_string(),
            asset: row.asset,
            chain: title_chain(&row.chain),
            snapshot_ts: row.snapshot_utc,
            total_observed_dex_liquidity_usd: row.total_liquidity_usd,
            usdc_pair_liquidity_usd: row.usdc_liquidity_usd,
            usdc_pair_share: row.usdc_share,
            usd_stable_pair_share: row.usd_stable_share,
            local_stable_pair_share: "0".to_string(),
            eur_stable_pair_share: row.eur_stable_share,
            weth_pair_share: row.weth_share,
            other_pair_share: row.other_share,
            top_pair_tvl_usd: row.top_pair_liquidity_usd,
            top_pair_share: row.top_pair_share,
            n_pools: row.pool_count,
        };
        writer.serialize(out)?;
        count += 1;
    }
    writer.flush()?;
    Ok(count)
}

#[derive(Debug, Deserialize)]
struct LiquidityPairInput {
    asset: String,
    chain: String,
    snapshot_utc: String,
    pool_address: String,
    dex: String,
    token0_symbol: String,
    token1_symbol: String,
    reserve_usd: String,
    counterpart_class: String,
    source: String,
}

#[derive(Debug, Serialize)]
struct DependencyEdgeRow {
    snapshot_ts: String,
    chain: String,
    source_asset: String,
    target_asset: String,
    source_node_id: String,
    target_node_id: String,
    pair_address: String,
    dex: String,
    tvl_usd: String,
    source_counterpart_class: String,
    target_counterpart_class: String,
    edge_type: String,
    edge_weight_log_tvl: String,
    source_asset_pool_share: String,
    target_asset_pool_share: String,
    confidence: String,
    caveat: String,
    source_file: String,
}

fn write_dependency_edges(input: &Path, output_dir: &Path) -> Result<usize> {
    let mut reader = csv::Reader::from_path(input)?;
    let mut rows = Vec::new();
    let mut totals: HashMap<(String, String), f64> = HashMap::new();
    for row in reader.deserialize::<LiquidityPairInput>() {
        let row = row?;
        let tvl = parse_f64(&row.reserve_usd);
        *totals
            .entry((row.asset.clone(), row.chain.clone()))
            .or_insert(0.0) += tvl;
        rows.push(row);
    }

    let mut writer = csv::Writer::from_path(output_dir.join("stablecoin_dependency_edges.csv"))?;
    let mut count = 0;
    for row in rows {
        let tvl = parse_f64(&row.reserve_usd);
        let total = totals
            .get(&(row.asset.clone(), row.chain.clone()))
            .copied()
            .unwrap_or(0.0);
        let share = if total > 0.0 { tvl / total } else { 0.0 };
        let target = target_asset(&row);
        writer.serialize(DependencyEdgeRow {
            snapshot_ts: row.snapshot_utc,
            chain: title_chain(&row.chain),
            source_asset: row.asset.clone(),
            target_asset: target.clone(),
            source_node_id: format!("{}::{}", row.asset, title_chain(&row.chain)),
            target_node_id: format!("{}::{}", target, title_chain(&row.chain)),
            pair_address: row.pool_address,
            dex: row.dex,
            tvl_usd: row.reserve_usd,
            source_counterpart_class: row.asset.clone(),
            target_counterpart_class: normalize_counterpart_class(&target, &row.counterpart_class),
            edge_type: "observed_dex_pool".to_string(),
            edge_weight_log_tvl: if tvl > 0.0 {
                format!("{:.8}", tvl.log10())
            } else {
                "0".to_string()
            },
            source_asset_pool_share: format!("{share:.8}"),
            target_asset_pool_share: String::new(),
            confidence: "observed_pool_snapshot".to_string(),
            caveat:
                "DexScreener coverage is not complete market coverage; counterpart classification may be heuristic."
                    .to_string(),
            source_file: format!("{} ({})", input.display(), row.source),
        })?;
        count += 1;
    }
    writer.flush()?;
    Ok(count)
}

#[derive(Debug, Deserialize)]
struct TokenConfig {
    asset: String,
    chain: String,
    contract_address: String,
}

fn load_config_addresses(dir: &Path) -> Result<HashMap<String, Vec<String>>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if !dir.exists() {
        return Ok(HashMap::new());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        let raw = fs::read_to_string(&path)?;
        let cfg: TokenConfig =
            serde_yaml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        out.entry(cfg.asset).or_default().push(format!(
            "{}:{}",
            title_chain(&cfg.chain),
            cfg.contract_address
        ));
    }
    Ok(out.into_iter().collect())
}

fn load_observed_liquidity(output_dir: &Path) -> Result<HashMap<String, f64>> {
    let path = output_dir.join("stablecoin_dependency_summary.csv");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let mut reader = csv::Reader::from_path(path)?;
    let mut out = HashMap::new();
    for row in reader.deserialize::<DependencySummaryRowForRead>() {
        let row = row?;
        *out.entry(row.asset).or_insert(0.0) += parse_f64(&row.total_observed_dex_liquidity_usd);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
struct DependencySummaryRowForRead {
    asset: String,
    total_observed_dex_liquidity_usd: String,
}

fn peg_currency(peg_type: &str) -> String {
    let peg = match peg_type {
        "peggedUSD" => "USD",
        "peggedEUR" => "EUR",
        "peggedSGD" => "SGD",
        "peggedJPY" => "JPY",
        "peggedREAL" => "BRL",
        "peggedTRY" => "TRY",
        "peggedCAD" => "CAD",
        "peggedAUD" => "AUD",
        "peggedCOP" => "COP",
        other => other.strip_prefix("pegged").unwrap_or(other),
    };
    peg.to_string()
}

fn native_circulating(asset: &DefiLlamaAsset) -> f64 {
    let peg_type = asset.peg_type.as_deref().unwrap_or_default();
    asset
        .circulating
        .as_ref()
        .and_then(|m| m.get(peg_type))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn usd_value(asset: &DefiLlamaAsset) -> f64 {
    native_circulating(asset) * asset.price.unwrap_or(1.0)
}

fn format_chains(asset: &DefiLlamaAsset, limit: usize) -> String {
    let chains = asset
        .chains
        .clone()
        .or_else(|| {
            asset
                .chain_circulating
                .as_ref()
                .map(|m| m.keys().cloned().collect())
        })
        .unwrap_or_default();
    let suffix = if chains.len() > limit {
        format!("; +{} more", chains.len() - limit)
    } else {
        String::new()
    };
    format!(
        "{}{}",
        chains
            .iter()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .join(", "),
        suffix
    )
}

fn country_zone(peg: &str) -> &'static str {
    match peg {
        "USD" => "United States / USD zone",
        "EUR" => "Euro area / EUR zone",
        "SGD" => "Singapore / SGD",
        "JPY" => "Japan / JPY",
        "BRL" => "Brazil / BRL",
        "TRY" => "Turkey / TRY",
        "CAD" => "Canada / CAD",
        "AUD" => "Australia / AUD",
        "COP" => "Colombia / COP",
        _ => "Currency zone",
    }
}

fn issuer(symbol: &str, fallback: &str) -> &'static str {
    match symbol {
        "USDT" => "Tether",
        "USDC" => "Circle",
        "USDS" => "Sky",
        "USD1" => "World Liberty Financial",
        "DAI" => "Sky / MakerDAO",
        "USDe" => "Ethena",
        "PYUSD" => "PayPal / Paxos",
        "RLUSD" => "Ripple",
        "USDTB" => "Ethena",
        "TUSD" => "Techteryx / TrueUSD",
        "FDUSD" => "First Digital",
        "FRAX" => "Frax",
        "EURC" => "Circle",
        "EURI" => "Banking Circle",
        "EURE" => "Monerium",
        "EURS" => "STASIS",
        "EURA" => "Angle",
        "EUROe" => "Membrane Finance",
        "XSGD" => "StraitsX",
        "GYEN" => "GMO Trust",
        "BRLA" => "BRLA Digital",
        "CADC" => "PayTrie / Stablecorp",
        "AUDD" => "AUDC Pty Ltd",
        "COPm" => "Mento",
        _ => Box::leak(fallback.to_string().into_boxed_str()),
    }
}

fn token_category(symbol: &str) -> &'static str {
    match symbol {
        "USDS" | "DAI" | "USDe" | "FRAX" => "synthetic_crypto_collateral_or_yield_bearing",
        "EURC" | "EURI" | "EURE" | "EURS" | "EURA" | "EUROe" | "XSGD" | "GYEN" | "BRLA"
        | "CADC" | "AUDD" | "COPm" => "non_usd_fiat_linked_rail",
        _ => "global_usd_rail",
    }
}

fn target_asset(row: &LiquidityPairInput) -> String {
    if row.token0_symbol.eq_ignore_ascii_case(&row.asset) {
        row.token1_symbol.clone()
    } else {
        row.token0_symbol.clone()
    }
}

fn normalize_counterpart_class(target: &str, class: &str) -> String {
    let t = target.to_uppercase();
    if t == "USDC" {
        "USDC".to_string()
    } else if matches!(
        t.as_str(),
        "USDT" | "USDS" | "DAI" | "FRAX" | "PYUSD" | "USD1"
    ) {
        "USDT/USD-stable".to_string()
    } else if class.eq_ignore_ascii_case("WETH_ETH") || t == "WETH" {
        "WETH".to_string()
    } else if class.to_ascii_lowercase().contains("eur") {
        "EUR-stable".to_string()
    } else {
        "Other".to_string()
    }
}

fn top_counterpart(top_pair: &str, asset: &str) -> String {
    top_pair
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .split('/')
        .find(|part| !part.eq_ignore_ascii_case(asset))
        .unwrap_or_default()
        .to_string()
}

fn count_counterpart_classes(row: &PairDependenceInput) -> String {
    [
        &row.usdc_share,
        &row.usd_stable_share,
        &row.eur_stable_share,
        &row.weth_share,
        &row.other_share,
    ]
    .iter()
    .filter(|value| parse_f64(value) > 0.0)
    .count()
    .to_string()
}

fn title_chain(chain: &str) -> String {
    let mut chars = chain.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn parse_f64(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

fn comma0(value: f64) -> String {
    let rounded = value.round() as i128;
    let sign = if rounded < 0 { "-" } else { "" };
    let digits = rounded.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{sign}{}", out.chars().rev().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn maps_peg_currency() {
        assert_eq!(peg_currency("peggedUSD"), "USD");
        assert_eq!(peg_currency("peggedREAL"), "BRL");
        assert_eq!(peg_currency("peggedXYZ"), "XYZ");
    }

    #[test]
    fn formats_commas() {
        assert_eq!(comma0(76_069_354_478.2), "76,069,354,478");
        assert_eq!(comma0(999.4), "999");
    }

    #[test]
    fn extracts_top_counterpart() {
        assert_eq!(top_counterpart("EURC/USDC (uniswap)", "EURC"), "USDC");
        assert_eq!(top_counterpart("USDC/WETH (uniswap)", "USDC"), "WETH");
    }

    #[test]
    fn normalizes_counterpart_class() {
        assert_eq!(normalize_counterpart_class("USDC", "USDC"), "USDC");
        assert_eq!(normalize_counterpart_class("WETH", "WETH_ETH"), "WETH");
        assert_eq!(
            normalize_counterpart_class("USDT", "USD_STABLE"),
            "USDT/USD-stable"
        );
    }

    #[test]
    fn formats_asset_values_and_chains() {
        let asset = DefiLlamaAsset {
            symbol: Some("TEST".to_string()),
            name: Some("Test USD".to_string()),
            peg_type: Some("peggedUSD".to_string()),
            peg_mechanism: Some("fiat-backed".to_string()),
            circulating: Some(HashMap::from([(
                "peggedUSD".to_string(),
                Value::from(1_500_000.0),
            )])),
            price: Some(0.999),
            chains: Some(vec![
                "Ethereum".to_string(),
                "Base".to_string(),
                "Arbitrum".to_string(),
            ]),
            chain_circulating: None,
        };

        assert_eq!(native_circulating(&asset), 1_500_000.0);
        assert_eq!(usd_value(&asset), 1_498_500.0);
        assert_eq!(format_chains(&asset, 2), "Ethereum, Base; +1 more");
        assert_eq!(country_zone("SGD"), "Singapore / SGD");
        assert_eq!(country_zone("ZZZ"), "Currency zone");
        assert_eq!(issuer("USDC", "fallback"), "Circle");
        assert_eq!(issuer("UNKNOWN", "fallback"), "fallback");
        assert_eq!(
            token_category("USDe"),
            "synthetic_crypto_collateral_or_yield_bearing"
        );
        assert_eq!(token_category("XSGD"), "non_usd_fiat_linked_rail");
        assert_eq!(token_category("USDT"), "global_usd_rail");
    }

    #[test]
    fn derives_target_asset_and_counts_classes() {
        let pair = LiquidityPairInput {
            asset: "EURC".to_string(),
            chain: "base".to_string(),
            snapshot_utc: "2026-05-21T00:00:00Z".to_string(),
            pool_address: "0xpool".to_string(),
            dex: "aerodrome".to_string(),
            token0_symbol: "USDC".to_string(),
            token1_symbol: "EURC".to_string(),
            reserve_usd: "1000".to_string(),
            counterpart_class: "USDC".to_string(),
            source: "dexscreener".to_string(),
        };
        assert_eq!(target_asset(&pair), "USDC");
        assert_eq!(title_chain("base"), "Base");
        assert_eq!(title_chain(""), "");
        assert_eq!(parse_f64("bad"), 0.0);

        let summary = PairDependenceInput {
            asset: "EURC".to_string(),
            chain: "base".to_string(),
            snapshot_utc: "2026-05-21T00:00:00Z".to_string(),
            total_liquidity_usd: "1000".to_string(),
            usdc_liquidity_usd: "600".to_string(),
            usdc_share: "0.6".to_string(),
            usd_stable_share: "0.6".to_string(),
            eur_stable_share: "0".to_string(),
            weth_share: "0.3".to_string(),
            other_share: "bad".to_string(),
            top_pair: "EURC/USDC (uniswap)".to_string(),
            top_pair_liquidity_usd: "600".to_string(),
            top_pair_share: "0.6".to_string(),
            pool_count: "2".to_string(),
            notes: None,
        };
        assert_eq!(count_counterpart_classes(&summary), "3");
    }

    #[test]
    fn writes_dependency_package_from_csv_fixtures() {
        let temp = temp_dir("stablecoin-map-writers");
        let input_dir = temp.join("input");
        let output_dir = temp.join("output");
        fs::create_dir_all(&input_dir).unwrap();
        fs::create_dir_all(&output_dir).unwrap();

        let summary_path = input_dir.join("summary.csv");
        fs::write(
            &summary_path,
            concat!(
                "asset,chain,snapshot_utc,pool_count,total_liquidity_usd,usd_stable_liquidity_usd,usdc_liquidity_usd,usdt_liquidity_usd,weth_liquidity_usd,eur_stable_liquidity_usd,btc_liquidity_usd,other_liquidity_usd,usd_stable_share,usdc_share,usdt_share,weth_share,eur_stable_share,btc_share,other_share,top_pair,top_pair_liquidity_usd,top_pair_share,notes\n",
                "EURC,base,2026-05-21T00:00:00Z,2,1000,700,600,100,200,0,0,100,0.7,0.6,0.1,0.2,0,0,0.1,EURC/USDC (uniswap),600,0.6,\n"
            ),
        )
        .unwrap();
        assert_eq!(
            write_dependency_summary(&summary_path, &output_dir).unwrap(),
            1
        );

        let edges_path = input_dir.join("pairs.csv");
        fs::write(
            &edges_path,
            concat!(
                "asset,chain,snapshot_utc,pool_address,dex,token0,token1,token0_symbol,token1_symbol,reserve_usd,volume_24h_usd,is_usd_stable_pair,is_usdc_pair,is_usdt_pair,is_weth_pair,is_eur_stable_pair,is_btc_pair,counterpart_class,source\n",
                "EURC,base,2026-05-21T00:00:00Z,0xpool1,uniswap,0xeurc,0xusdc,EURC,USDC,600,10,true,true,false,false,false,false,USDC,dexscreener\n",
                "EURC,base,2026-05-21T00:00:00Z,0xpool2,uniswap,0xeurc,0xweth,EURC,WETH,400,5,false,false,false,true,false,false,WETH_ETH,dexscreener\n"
            ),
        )
        .unwrap();
        assert_eq!(write_dependency_edges(&edges_path, &output_dir).unwrap(), 2);

        let mut summary_reader =
            csv::Reader::from_path(output_dir.join("stablecoin_dependency_summary.csv")).unwrap();
        let summary_rows = summary_reader
            .deserialize::<HashMap<String, String>>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(summary_rows[0]["deployment_id"], "eurc_base");
        assert_eq!(summary_rows[0]["chain"], "Base");
        assert_eq!(summary_rows[0]["top_counterpart"], "USDC");
        assert_eq!(summary_rows[0]["n_counterpart_classes"], "4");

        let mut edge_reader =
            csv::Reader::from_path(output_dir.join("stablecoin_dependency_edges.csv")).unwrap();
        let edge_rows = edge_reader
            .deserialize::<HashMap<String, String>>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(edge_rows.len(), 2);
        assert_eq!(edge_rows[0]["target_asset"], "USDC");
        assert_eq!(edge_rows[0]["source_asset_pool_share"], "0.60000000");
        assert_eq!(edge_rows[1]["target_counterpart_class"], "WETH");

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn run_skip_network_writes_local_package() {
        let temp = temp_dir("stablecoin-map-run");
        let output_dir = temp.join("output");
        fs::create_dir_all(&temp).unwrap();
        let summary_path = temp.join("summary.csv");
        let pairs_path = temp.join("pairs.csv");
        fs::write(
            &summary_path,
            concat!(
                "asset,chain,snapshot_utc,pool_count,total_liquidity_usd,usd_stable_liquidity_usd,usdc_liquidity_usd,usdt_liquidity_usd,weth_liquidity_usd,eur_stable_liquidity_usd,btc_liquidity_usd,other_liquidity_usd,usd_stable_share,usdc_share,usdt_share,weth_share,eur_stable_share,btc_share,other_share,top_pair,top_pair_liquidity_usd,top_pair_share,notes\n",
                "XSGD,polygon,2026-05-21T00:00:00Z,1,500,400,400,0,0,0,0,100,0.8,0.8,0,0,0,0,0.2,XSGD/USDC (quickswap),400,0.8,fixture note\n"
            ),
        )
        .unwrap();
        fs::write(
            &pairs_path,
            concat!(
                "asset,chain,snapshot_utc,pool_address,dex,token0,token1,token0_symbol,token1_symbol,reserve_usd,volume_24h_usd,is_usd_stable_pair,is_usdc_pair,is_usdt_pair,is_weth_pair,is_eur_stable_pair,is_btc_pair,counterpart_class,source\n",
                "XSGD,polygon,2026-05-21T00:00:00Z,0xpool,quickswap,0xxsgd,0xusdc,XSGD,USDC,500,10,true,true,false,false,false,false,USDC,dexscreener\n"
            ),
        )
        .unwrap();

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(run(PackageOptions {
                output_dir: output_dir.clone(),
                dependency_summary: summary_path,
                liquidity_pairs: pairs_path,
                artemis_start: "2026-04-28".to_string(),
                artemis_end: "2026-05-27".to_string(),
                skip_network: true,
            }))
            .unwrap();

        assert!(output_dir
            .join("stablecoin_dependency_summary.csv")
            .is_file());
        assert!(output_dir.join("stablecoin_dependency_edges.csv").is_file());
        assert!(!output_dir
            .join("global_stablecoin_inventory_v1.csv")
            .exists());

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn builds_inventory_rows_from_payload_and_local_artifacts() {
        let temp = temp_dir("stablecoin-map-inventory");
        let output_dir = temp.join("output");
        let config_dir = temp.join("configs");
        fs::create_dir_all(&output_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            output_dir.join("stablecoin_dependency_summary.csv"),
            concat!(
                "asset,total_observed_dex_liquidity_usd\n",
                "USDC,1000\n",
                "USDC,250.5\n",
                "XSGD,not-a-number\n"
            ),
        )
        .unwrap();
        fs::write(
            config_dir.join("usdc.base.yml"),
            concat!(
                "asset: USDC\n",
                "chain: base\n",
                "contract_address: \"0xabc\"\n"
            ),
        )
        .unwrap();
        fs::write(config_dir.join("ignore.txt"), "not yaml").unwrap();

        let payload = DefiLlamaResponse {
            pegged_assets: vec![
                DefiLlamaAsset {
                    symbol: Some("USDC".to_string()),
                    name: Some("USD Coin small".to_string()),
                    peg_type: Some("peggedUSD".to_string()),
                    peg_mechanism: Some("fiat-backed".to_string()),
                    circulating: Some(HashMap::from([(
                        "peggedUSD".to_string(),
                        Value::from(10.0),
                    )])),
                    price: Some(1.0),
                    chains: Some(vec!["Base".to_string()]),
                    chain_circulating: None,
                },
                DefiLlamaAsset {
                    symbol: Some("USDC".to_string()),
                    name: Some("USD Coin".to_string()),
                    peg_type: Some("peggedUSD".to_string()),
                    peg_mechanism: Some("fiat-backed".to_string()),
                    circulating: Some(HashMap::from([(
                        "peggedUSD".to_string(),
                        Value::from(2_000_000.0),
                    )])),
                    price: Some(1.0),
                    chains: Some((0..13).map(|i| format!("Chain{i}")).collect()),
                    chain_circulating: None,
                },
                DefiLlamaAsset {
                    symbol: Some("EURI".to_string()),
                    name: Some("Tiny Euro".to_string()),
                    peg_type: Some("peggedEUR".to_string()),
                    peg_mechanism: None,
                    circulating: Some(HashMap::from([(
                        "peggedEUR".to_string(),
                        Value::from(50_000.0),
                    )])),
                    price: Some(1.0),
                    chains: None,
                    chain_circulating: Some(HashMap::from([(
                        "Gnosis".to_string(),
                        Value::from(1.0),
                    )])),
                },
                DefiLlamaAsset {
                    symbol: None,
                    name: Some("No symbol".to_string()),
                    peg_type: Some("peggedUSD".to_string()),
                    peg_mechanism: None,
                    circulating: None,
                    price: None,
                    chains: None,
                    chain_circulating: None,
                },
            ],
        };

        let rows = build_inventory_rows(
            payload,
            &output_dir,
            &config_dir,
            "2026-05-30T00:00:00Z".to_string(),
        )
        .unwrap();
        assert_eq!(rows.len(), 2);

        let usdc = rows.iter().find(|row| row.symbol == "USDC").unwrap();
        assert_eq!(usdc.name, "USD Coin");
        assert_eq!(usdc.confidence_grade, "high");
        assert_eq!(usdc.contract_addresses_if_available, "Base:0xabc");
        assert_eq!(
            usdc.observed_dex_liquidity_if_available,
            "$1,251 observed DEX TVL in project DexScreener snapshot"
        );
        assert!(usdc.chains_or_deployments.ends_with("; +1 more"));

        let euri = rows.iter().find(|row| row.symbol == "EURI").unwrap();
        assert_eq!(euri.confidence_grade, "low");
        assert_eq!(euri.chains_or_deployments, "Gnosis");
        assert!(euri
            .notes
            .contains("Peg mechanism reported by DefiLlama: not reported."));

        write_inventory(&rows, &output_dir).unwrap();
        let mut reader =
            csv::Reader::from_path(output_dir.join("global_stablecoin_inventory_v1.csv")).unwrap();
        let written = reader
            .deserialize::<InventoryRow>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(written.len(), 2);

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn builds_transfer_rows_for_supported_and_unavailable_tokens() {
        let inventory = vec![
            InventoryRow {
                symbol: "USDC".to_string(),
                name: "USD Coin".to_string(),
                peg_currency: "USD".to_string(),
                country_or_currency_zone: "United States / USD zone".to_string(),
                issuer_or_protocol: "Circle".to_string(),
                market_cap_or_circulating_supply: String::new(),
                supply_source: String::new(),
                chains_or_deployments: String::new(),
                contract_addresses_if_available: String::new(),
                observed_dex_liquidity_if_available: String::new(),
                source_url_or_artifact: String::new(),
                source_timestamp: String::new(),
                confidence_grade: String::new(),
                notes: String::new(),
            },
            InventoryRow {
                symbol: "XSGD".to_string(),
                name: "XSGD".to_string(),
                peg_currency: "SGD".to_string(),
                country_or_currency_zone: "Singapore / SGD".to_string(),
                issuer_or_protocol: "StraitsX".to_string(),
                market_cap_or_circulating_supply: String::new(),
                supply_source: String::new(),
                chains_or_deployments: String::new(),
                contract_addresses_if_available: String::new(),
                observed_dex_liquidity_if_available: String::new(),
                source_url_or_artifact: String::new(),
                source_timestamp: String::new(),
                confidence_grade: String::new(),
                notes: String::new(),
            },
        ];
        let payload = serde_json::json!({
            "data": {
                "symbols": {
                    "usdc": {
                        ARTEMIS_METRIC: [
                            {"val": 10.0},
                            {"val": 20.5},
                            {"val": null},
                            {"other": 99.0}
                        ]
                    },
                    "xsgd": {
                        ARTEMIS_METRIC: "unsupported"
                    }
                }
            }
        });

        let rows = build_transfer_volume_rows(
            payload,
            &inventory,
            "2026-04-28",
            "2026-05-27",
            "https://example.test/source".to_string(),
            "2026-05-30T00:00:00Z".to_string(),
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].total_30d_transfer_volume, "30.500000");
        assert_eq!(rows[0].non_null_days, 2);
        assert_eq!(rows[0].missing_days, 28);
        assert_eq!(rows[0].token_category, "global_usd_rail");
        assert_eq!(rows[1].total_30d_transfer_volume, "");
        assert_eq!(rows[1].token_category, "non_usd_fiat_linked_rail");
        assert!(rows[1]
            .notes
            .contains("Metric unavailable response: \"unsupported\""));

        let temp = temp_dir("stablecoin-map-transfer");
        fs::create_dir_all(&temp).unwrap();
        write_transfer_volume(&rows, &temp).unwrap();
        assert!(temp
            .join("stablecoin_transfer_volume_selected_rails_v1.csv")
            .is_file());
        fs::remove_dir_all(temp).unwrap();
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }
}
