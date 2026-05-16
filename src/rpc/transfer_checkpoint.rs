//! Per-chain checkpoints under `out/<asset>/runs/<run_id>/checkpoint/`.
//! Chain-level: full chain done → `transfers_<chain>.csv` + `chain_<chain>.json`.
//! Chunk-level (in-flight): each successful `eth_getLogs` chunk → `fetch_progress_<chain>.json` + append `fetch_partial_<chain>.csv`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use crate::decode::TransferEvent;

/// Serialized per completed chain (`checkpoint/chain_<name>.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointChainBundle {
    pub supply: crate::rpc::transfer_audit::SupplyAuditRow,
    pub qa: crate::rpc::transfer_audit::QaChain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainSpecRecord {
    pub chain: String,
    pub from_block: u64,
    pub to_block_requested: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointManifest {
    pub schema: String,
    pub asset: String,
    pub run_id: String,
    pub started_at: String,
    pub chunk_size: u64,
    pub per_chain_spans: bool,
    pub chain_specs: Vec<ChainSpecRecord>,
    pub completed_chains: Vec<String>,
}

impl CheckpointManifest {
    pub const SCHEMA: &'static str = "transfer-audit-checkpoint-v1";

    pub fn new(
        asset: &str,
        run_id: &str,
        started_at: &str,
        chunk_size: u64,
        per_chain_spans: bool,
        chain_specs: Vec<ChainSpecRecord>,
    ) -> Self {
        Self {
            schema: Self::SCHEMA.to_string(),
            asset: asset.to_uppercase(),
            run_id: run_id.to_string(),
            started_at: started_at.to_string(),
            chunk_size,
            per_chain_spans,
            chain_specs,
            completed_chains: Vec::new(),
        }
    }
}

/// In-flight log fetch for one chain (`checkpoint/fetch_progress_<chain>.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchChunkProgress {
    pub schema: String,
    pub chain: String,
    pub contract_address: String,
    pub from_block: u64,
    pub to_block: u64,
    pub chunk_size: u64,
    /// Inclusive end block of the last successful `eth_getLogs` chunk.
    pub last_fetched_through: u64,
    pub chunks_done: u64,
    pub total_chunks: u64,
    pub logs_fetched: usize,
}

impl FetchChunkProgress {
    pub const SCHEMA: &'static str = "transfer-audit-fetch-chunk-v1";

    pub fn resume_from_block(&self) -> u64 {
        self.last_fetched_through.saturating_add(1)
    }

    pub fn is_complete(&self) -> bool {
        self.last_fetched_through >= self.to_block
    }
}

pub fn checkpoint_root(out_dir: &Path) -> PathBuf {
    out_dir.join("checkpoint")
}

pub fn manifest_path(out_dir: &Path) -> PathBuf {
    checkpoint_root(out_dir).join("manifest.json")
}

fn transfers_path(out_dir: &Path, chain: &str) -> PathBuf {
    checkpoint_root(out_dir).join(format!("transfers_{chain}.csv"))
}

fn chain_bundle_path(out_dir: &Path, chain: &str) -> PathBuf {
    checkpoint_root(out_dir).join(format!("chain_{chain}.json"))
}

pub fn fetch_progress_path(out_dir: &Path, chain: &str) -> PathBuf {
    checkpoint_root(out_dir).join(format!("fetch_progress_{chain}.json"))
}

pub fn fetch_partial_path(out_dir: &Path, chain: &str) -> PathBuf {
    checkpoint_root(out_dir).join(format!("fetch_partial_{chain}.csv"))
}

pub fn clear_checkpoint_dir(out_dir: &Path) -> Result<()> {
    let root = checkpoint_root(out_dir);
    if root.exists() {
        std::fs::remove_dir_all(&root).with_context(|| format!("remove {}", root.display()))?;
    }
    Ok(())
}

pub fn clear_chain_fetch_progress(out_dir: &Path, chain: &str) -> Result<()> {
    for p in [fetch_progress_path(out_dir, chain), fetch_partial_path(out_dir, chain)] {
        if p.exists() {
            std::fs::remove_file(&p).ok();
        }
    }
    Ok(())
}

pub fn load_manifest(out_dir: &Path) -> Result<Option<CheckpointManifest>> {
    let path = manifest_path(out_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
    let m: CheckpointManifest =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    if m.schema != CheckpointManifest::SCHEMA {
        anyhow::bail!(
            "unsupported checkpoint schema {:?} in {}; use --fresh to restart",
            m.schema,
            path.display()
        );
    }
    Ok(Some(m))
}

pub fn save_manifest(out_dir: &Path, manifest: &CheckpointManifest) -> Result<()> {
    let root = checkpoint_root(out_dir);
    std::fs::create_dir_all(&root)?;
    let path = manifest_path(out_dir);
    std::fs::write(&path, serde_json::to_string_pretty(manifest)?)?;
    Ok(())
}

pub fn validate_manifest_matches(
    manifest: &CheckpointManifest,
    asset: &str,
    run_id: &str,
    chunk_size: u64,
    per_chain_spans: bool,
    chain_specs: &[ChainSpecRecord],
) -> Result<()> {
    if manifest.asset.to_uppercase() != asset.to_uppercase() {
        anyhow::bail!(
            "checkpoint asset {:?} != --asset {:?}; use --fresh or a different --run-id",
            manifest.asset,
            asset
        );
    }
    if manifest.run_id != run_id {
        anyhow::bail!(
            "checkpoint run_id {:?} != --run-id {:?}; use --fresh or matching --run-id",
            manifest.run_id,
            run_id
        );
    }
    if manifest.chunk_size != chunk_size {
        anyhow::bail!(
            "checkpoint chunk_size {} != current {} ; use --fresh",
            manifest.chunk_size,
            chunk_size
        );
    }
    if manifest.per_chain_spans != per_chain_spans {
        anyhow::bail!("checkpoint per_chain_spans mismatch; use --fresh");
    }
    let mut want: Vec<_> = chain_specs.to_vec();
    let mut have = manifest.chain_specs.clone();
    want.sort_by(|a, b| a.chain.cmp(&b.chain));
    have.sort_by(|a, b| a.chain.cmp(&b.chain));
    if want != have {
        anyhow::bail!(
            "checkpoint window specs differ from current CLI args; use --fresh.\n  checkpoint: {:?}\n  current:    {:?}",
            have,
            want
        );
    }
    Ok(())
}

pub fn completed_set(manifest: &CheckpointManifest) -> HashSet<String> {
    manifest.completed_chains.iter().cloned().collect()
}

fn addr_norm(a: &str) -> String {
    a.trim_start_matches("0x").to_lowercase()
}

pub fn load_fetch_progress(
    out_dir: &Path,
    chain: &str,
    contract_address: &str,
    from_block: u64,
    to_block: u64,
    chunk_size: u64,
) -> Result<Option<FetchChunkProgress>> {
    let path = fetch_progress_path(out_dir, chain);
    if !path.exists() {
        return Ok(None);
    }
    let p: FetchChunkProgress = serde_json::from_str(
        &std::fs::read_to_string(&path).with_context(|| path.display().to_string())?,
    )
    .with_context(|| format!("parse {}", path.display()))?;
    if p.schema != FetchChunkProgress::SCHEMA {
        anyhow::bail!("unsupported fetch progress schema in {}", path.display());
    }
    if p.chain != chain
        || p.from_block != from_block
        || p.to_block != to_block
        || p.chunk_size != chunk_size
        || addr_norm(&p.contract_address) != addr_norm(contract_address)
    {
        anyhow::bail!(
            "fetch checkpoint for {:?} does not match current window; use --fresh or delete {}",
            chain,
            path.display()
        );
    }
    Ok(Some(p))
}

pub fn save_fetch_progress(out_dir: &Path, progress: &FetchChunkProgress) -> Result<()> {
    std::fs::create_dir_all(checkpoint_root(out_dir))?;
    let path = fetch_progress_path(out_dir, &progress.chain);
    std::fs::write(&path, serde_json::to_string_pretty(progress)?)?;
    Ok(())
}

pub fn load_fetch_partial_events(out_dir: &Path, chain: &str) -> Result<Vec<TransferEvent>> {
    let path = fetch_partial_path(out_dir, chain);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    let mut rdr = csv::Reader::from_path(&path)?;
    for row in rdr.deserialize::<TransferEvent>() {
        events.push(row?);
    }
    Ok(events)
}

pub fn append_fetch_partial_events(out_dir: &Path, chain: &str, new_rows: &[TransferEvent]) -> Result<()> {
    if new_rows.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(checkpoint_root(out_dir))?;
    let path = fetch_partial_path(out_dir, chain);
    let file_exists = path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let mut wtr = csv::WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(&mut file);
    for ev in new_rows {
        wtr.serialize(ev)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn load_completed_chain(
    out_dir: &Path,
    chain: &str,
) -> Result<(Vec<TransferEvent>, CheckpointChainBundle)> {
    let csv_path = transfers_path(out_dir, chain);
    let json_path = chain_bundle_path(out_dir, chain);
    if !csv_path.exists() || !json_path.exists() {
        anyhow::bail!(
            "checkpoint for chain {:?} incomplete (missing transfers or chain bundle); use --fresh",
            chain
        );
    }
    let mut events = Vec::new();
    let mut rdr = csv::Reader::from_path(&csv_path)?;
    for row in rdr.deserialize::<TransferEvent>() {
        events.push(row?);
    }
    let bundle: CheckpointChainBundle = serde_json::from_str(
        &std::fs::read_to_string(&json_path).with_context(|| json_path.display().to_string())?,
    )
    .with_context(|| format!("parse {}", json_path.display()))?;
    Ok((events, bundle))
}

pub fn save_completed_chain(
    out_dir: &Path,
    manifest: &mut CheckpointManifest,
    chain: &str,
    events: &[TransferEvent],
    bundle: &CheckpointChainBundle,
) -> Result<()> {
    let root = checkpoint_root(out_dir);
    std::fs::create_dir_all(&root)?;
    clear_chain_fetch_progress(out_dir, chain)?;
    let csv_path = transfers_path(out_dir, chain);
    let mut wtr = csv::Writer::from_path(&csv_path)?;
    for ev in events {
        wtr.serialize(ev)?;
    }
    wtr.flush()?;
    let json_path = chain_bundle_path(out_dir, chain);
    std::fs::write(&json_path, serde_json::to_string_pretty(bundle)?)?;
    if !manifest.completed_chains.iter().any(|c| c == chain) {
        manifest.completed_chains.push(chain.to_string());
        manifest.completed_chains.sort();
    }
    save_manifest(out_dir, manifest)?;
    println!(
        "[{}] chain checkpoint saved ({} transfer rows) — safe to stop; re-run same --run-id to resume",
        chain.to_uppercase(),
        events.len()
    );
    Ok(())
}

pub fn remove_checkpoint_dir(out_dir: &Path) -> Result<()> {
    clear_checkpoint_dir(out_dir)
}

/// Inclusive chunk count for `from_block..=to_block` with given `chunk_size`.
pub fn count_chunks(from_block: u64, to_block: u64, chunk_size: u64) -> u64 {
    if to_block < from_block || chunk_size == 0 {
        return 0;
    }
    let span = to_block - from_block + 1;
    span.div_ceil(chunk_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;
    use crate::decode::TransferEvent;
    use std::path::PathBuf;

    fn tmp_out(suffix: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "stablecoin_audit_cp_{}_{suffix}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn sample_event(chain: &str) -> TransferEvent {
        TransferEvent {
            chain: chain.into(),
            contract_address: "0xabc".into(),
            block_number: 10,
            tx_hash: "0xtx".into(),
            log_index: 0,
            from: "0x0".into(),
            to: "0x1".into(),
            value_raw: "1".into(),
            value_decimal: "0.000001".into(),
            kind: "transfer".into(),
            value_u256: U256::from(1u64),
        }
    }

    fn sample_bundle(chain: &str) -> CheckpointChainBundle {
        let json = format!(
            r#"{{
              "supply": {{
                "chain": "{chain}",
                "chain_id": 1,
                "contract_address": "0xabc",
                "from_block": 100,
                "resolved_to_block": 200,
                "to_block_requested": "200",
                "chunk_size": 500,
                "transfer_event_count": 1,
                "active_senders": 1,
                "active_recipients": 1,
                "mint_count": 0,
                "burn_count": 0,
                "plain_transfer_count": 1,
                "sum_mints_raw": "0",
                "sum_burns_raw": "0",
                "total_supply_at_start_minus_1_provenance": "on-chain",
                "metadata_call_pass": true,
                "historical_supply_pass": true,
                "duplicate_count": 0,
                "full_decode_error_count": 0
              }},
              "qa": {{
                "chain": "{chain}",
                "chain_id": 1,
                "contract_address": "0xabc",
                "from_block": 100,
                "resolved_to_block": 200,
                "gates": {{
                  "metadata_call_pass": "PASS",
                  "historical_supply_pass": "PASS",
                  "no_duplicate_logs_pass": "PASS",
                  "transfer_decode_pass": "PASS",
                  "supply_invariant_pass": "PASS",
                  "provenance_stamped": "PASS"
                }},
                "duplicate_count": 0,
                "full_decode_error_count": 0,
                "errors": []
              }}
            }}"#
        );
        serde_json::from_str(&json).expect("test bundle json")
    }

    #[test]
    fn count_chunks_single_and_multi() {
        assert_eq!(count_chunks(100, 100, 500), 1);
        assert_eq!(count_chunks(100, 599, 500), 1);
        assert_eq!(count_chunks(100, 600, 500), 2);
        assert_eq!(count_chunks(200, 100, 500), 0);
        assert_eq!(count_chunks(100, 200, 0), 0);
    }

    #[test]
    fn fetch_progress_resume_and_complete() {
        let p = FetchChunkProgress {
            schema: FetchChunkProgress::SCHEMA.into(),
            chain: "ethereum".into(),
            contract_address: "0xAbC".into(),
            from_block: 1,
            to_block: 1000,
            chunk_size: 500,
            last_fetched_through: 499,
            chunks_done: 1,
            total_chunks: 2,
            logs_fetched: 10,
        };
        assert_eq!(p.resume_from_block(), 500);
        assert!(!p.is_complete());
        let done = FetchChunkProgress {
            last_fetched_through: 1000,
            ..p
        };
        assert!(done.is_complete());
    }

    #[test]
    fn manifest_roundtrip_and_validate() {
        let out = tmp_out("manifest");
        let specs = vec![
            ChainSpecRecord {
                chain: "base".into(),
                from_block: 10,
                to_block_requested: "20".into(),
            },
            ChainSpecRecord {
                chain: "ethereum".into(),
                from_block: 100,
                to_block_requested: "200".into(),
            },
        ];
        let mut manifest = CheckpointManifest::new("usdc", "run_a", "2026-01-01T00:00:00Z", 500, true, specs.clone());
        save_manifest(&out, &manifest).unwrap();
        let loaded = load_manifest(&out).unwrap().expect("manifest exists");
        validate_manifest_matches(&loaded, "USDC", "run_a", 500, true, &specs).unwrap();
        assert!(validate_manifest_matches(&loaded, "USDC", "other", 500, true, &specs).is_err());
        manifest.completed_chains.push("ethereum".into());
        assert!(completed_set(&manifest).contains("ethereum"));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn fetch_progress_addr_case_insensitive() {
        let out = tmp_out("fetch_prog");
        let progress = FetchChunkProgress {
            schema: FetchChunkProgress::SCHEMA.into(),
            chain: "ethereum".into(),
            contract_address: "0xABCDEF".into(),
            from_block: 1,
            to_block: 10,
            chunk_size: 5,
            last_fetched_through: 5,
            chunks_done: 1,
            total_chunks: 2,
            logs_fetched: 3,
        };
        save_fetch_progress(&out, &progress).unwrap();
        let loaded = load_fetch_progress(&out, "ethereum", "0xabcdef", 1, 10, 5)
            .unwrap()
            .expect("progress");
        assert_eq!(loaded.last_fetched_through, 5);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn partial_events_append_and_load() {
        let out = tmp_out("partial");
        append_fetch_partial_events(&out, "base", &[sample_event("base")]).unwrap();
        append_fetch_partial_events(&out, "base", &[sample_event("base")]).unwrap();
        let rows = load_fetch_partial_events(&out, "base").unwrap();
        assert_eq!(rows.len(), 2);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn completed_chain_roundtrip() {
        let out = tmp_out("completed");
        let mut manifest = CheckpointManifest::new("usdc", "run_b", "t0", 500, false, vec![]);
        let events = vec![sample_event("ethereum")];
        let bundle = sample_bundle("ethereum");
        save_completed_chain(&out, &mut manifest, "ethereum", &events, &bundle).unwrap();
        let (loaded_events, _bundle) = load_completed_chain(&out, "ethereum").unwrap();
        assert_eq!(loaded_events.len(), 1);
        assert_eq!(loaded_events[0].chain, "ethereum");
        assert!(manifest.completed_chains.contains(&"ethereum".to_string()));
        let _ = std::fs::remove_dir_all(&out);
    }
}
