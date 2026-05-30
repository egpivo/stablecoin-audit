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

    let addresses = load_config_addresses(Path::new("configs/tokens"))?;
    let liquidity = load_observed_liquidity(output_dir)?;
    let fetched_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
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
    let extracted_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
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
    Ok(rows)
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
}
