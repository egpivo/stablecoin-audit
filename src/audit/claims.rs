//! Central claim catalog — canonical claim IDs, statements, and evidence kinds.

use std::collections::HashSet;

use crate::artifact::{ArtifactKind, ClaimBoundary, ClaimStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimDefinition {
    pub claim_id: &'static str,
    pub default_status: ClaimStatus,
    pub statement: &'static str,
    pub required_evidence_kinds: &'static [ArtifactKind],
    pub limitations: &'static [&'static str],
    pub warnings: &'static [&'static str],
    pub produced_by: &'static str,
}

pub const CLAIM_TRANSFER_ACTIVITY_RECONSTRUCTIBLE: &str = "transfer_activity_reconstructible";
pub const CLAIM_SUPPLY_SNAPSHOT_AVAILABLE: &str = "supply_snapshot_available";
pub const CLAIM_SUPPLY_RECONCILIATION_AVAILABLE: &str = "supply_reconciliation_available";
pub const CLAIM_CROSS_CHAIN_PER_DEPLOYMENT_COMPARISON: &str =
    "cross_chain_per_deployment_comparison";
pub const CLAIM_PER_CHAIN_TOTAL_SUPPLY_NOT_CIRCULATING: &str =
    "per_chain_totalSupply_not_circulating_supply";
pub const CLAIM_CIRCULATING_SUPPLY_NOT_VERIFIED: &str = "circulating_supply_not_verified";
pub const CLAIM_BRIDGE_BACKING_NOT_VERIFIED: &str =
    "bridge_backing_not_verified_without_bridge_collateral";
pub const CLAIM_FIAT_RESERVE_NOT_VERIFIED: &str = "fiat_reserve_not_verified";
pub const CLAIM_LIQUIDITY_EXPOSURE_NOT_MEASURED: &str = "liquidity_exposure_not_measured";
pub const CLAIM_PEG_STABILITY: &str = "peg_stability";
pub const CLAIM_REDEMPTION_CAPACITY: &str = "redemption_capacity";
pub const CLAIM_USER_GEOGRAPHY: &str = "user_geography";
pub const CLAIM_HOLDER_IDENTITY: &str = "holder_identity";
pub const CLAIM_ACTUAL_SWAP_ROUTING: &str = "actual_swap_routing";
pub const CLAIM_ISSUER_INTENT: &str = "issuer_intent";
pub const CLAIM_STRESS_TRANSMISSION: &str = "stress_transmission";

const CATALOG: &[ClaimDefinition] = &[
    ClaimDefinition {
        claim_id: CLAIM_TRANSFER_ACTIVITY_RECONSTRUCTIBLE,
        default_status: ClaimStatus::Conditional,
        statement: "Transfer logs in the configured block window were fetched and decoded for the scoped deployment(s).",
        required_evidence_kinds: &[
            ArtifactKind::CanonicalTransfers,
            ArtifactKind::TransferLog,
            ArtifactKind::QaReport,
        ],
        limitations: &[
            "Holds only when RPC fetch and decode gates PASS for the configured asset, chain, and block window.",
            "Transfers are not labeled by actor type or intent.",
        ],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_SUPPLY_SNAPSHOT_AVAILABLE,
        default_status: ClaimStatus::Conditional,
        statement: "Pinned totalSupply boundaries are available per chain for the audit window.",
        required_evidence_kinds: &[
            ArtifactKind::SupplySnapshots,
            ArtifactKind::SupplyAudit,
            ArtifactKind::Provenance,
            ArtifactKind::QaReport,
        ],
        limitations: &[
            "Snapshots use toolkit totalSupply() calls at pinned blocks; not circulating supply across chains.",
        ],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_SUPPLY_RECONCILIATION_AVAILABLE,
        default_status: ClaimStatus::Conditional,
        statement: "Mint/burn aggregates are compared to pinned totalSupply deltas per chain for the audit window.",
        required_evidence_kinds: &[ArtifactKind::SupplyAudit, ArtifactKind::QaReport],
        limitations: &[
            "Mint/burn sums use toolkit Transfer definitions (zero-address mint/burn).",
            "FAIL is not proof of fraud.",
        ],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_CROSS_CHAIN_PER_DEPLOYMENT_COMPARISON,
        default_status: ClaimStatus::Conditional,
        statement: "Per-deployment transfer-audit metrics are rolled up for cross-chain comparison on one asset schema.",
        required_evidence_kinds: &[
            ArtifactKind::CrossChainSummary,
            ArtifactKind::SupplyAudit,
        ],
        limitations: &[
            "Compares per-chain deployments on one schema; bridged inventory double-counts if summed as circulating supply.",
        ],
        warnings: &[],
        produced_by: "cross-chain-summary",
    },
    ClaimDefinition {
        claim_id: CLAIM_PER_CHAIN_TOTAL_SUPPLY_NOT_CIRCULATING,
        default_status: ClaimStatus::Conditional,
        statement: "Per-chain totalSupply(end) values are reported separately and must not be read as consolidated circulating supply.",
        required_evidence_kinds: &[
            ArtifactKind::CrossChainSummary,
            ArtifactKind::SupplyAudit,
        ],
        limitations: &[
            "Summing per-chain totalSupply(end) double-counts bridged or custodied inventory.",
        ],
        warnings: &[],
        produced_by: "cross-chain-summary",
    },
    ClaimDefinition {
        claim_id: CLAIM_CIRCULATING_SUPPLY_NOT_VERIFIED,
        default_status: ClaimStatus::Unsupported,
        statement: "Circulating supply across chains or through bridges is not verified by transfer-audit alone.",
        required_evidence_kinds: &[],
        limitations: &[
            "Per-chain totalSupply(end) is not circulating supply when inventory is bridged or held in custody.",
            "Summing per-chain totals double-counts bridged inventory.",
        ],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_BRIDGE_BACKING_NOT_VERIFIED,
        default_status: ClaimStatus::Unsupported,
        statement: "Bridge collateral, mint authority, and reserve backing are not verified without bridge-specific collateral evidence.",
        required_evidence_kinds: &[],
        limitations: &[
            "Cross-chain summary compares on-chain totals only; bridge attestations and reserve data are out of scope.",
        ],
        warnings: &[],
        produced_by: "cross-chain-summary",
    },
    ClaimDefinition {
        claim_id: CLAIM_FIAT_RESERVE_NOT_VERIFIED,
        default_status: ClaimStatus::Unsupported,
        statement: "Fiat reserve adequacy and bank attestation are not verified by on-chain transfer-audit evidence.",
        required_evidence_kinds: &[],
        limitations: &["No bank or attestation data in this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_LIQUIDITY_EXPOSURE_NOT_MEASURED,
        default_status: ClaimStatus::Unsupported,
        statement: "DEX pool depth, CEX liquidity, and oracle price exposure are not measured by this toolkit.",
        required_evidence_kinds: &[],
        limitations: &[
            "No DEX, CEX, or oracle depth series in transfer-audit or cross-chain-summary alone.",
        ],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_PEG_STABILITY,
        default_status: ClaimStatus::Unsupported,
        statement: "Peg or price stability is out of scope for transfer-audit.",
        required_evidence_kinds: &[],
        limitations: &["Out of scope for transfer-audit; not attested by this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_REDEMPTION_CAPACITY,
        default_status: ClaimStatus::Unsupported,
        statement: "Redemption capacity is out of scope for transfer-audit.",
        required_evidence_kinds: &[],
        limitations: &["Out of scope for transfer-audit; not attested by this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_USER_GEOGRAPHY,
        default_status: ClaimStatus::Unsupported,
        statement: "User geography is out of scope for transfer-audit.",
        required_evidence_kinds: &[],
        limitations: &["Out of scope for transfer-audit; not attested by this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_HOLDER_IDENTITY,
        default_status: ClaimStatus::Unsupported,
        statement: "Holder identity is out of scope for transfer-audit.",
        required_evidence_kinds: &[],
        limitations: &["Out of scope for transfer-audit; not attested by this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_ACTUAL_SWAP_ROUTING,
        default_status: ClaimStatus::Unsupported,
        statement: "Actual swap routing is out of scope for transfer-audit.",
        required_evidence_kinds: &[],
        limitations: &["Out of scope for transfer-audit; not attested by this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_ISSUER_INTENT,
        default_status: ClaimStatus::Unsupported,
        statement: "Issuer intent is out of scope for transfer-audit.",
        required_evidence_kinds: &[],
        limitations: &["Out of scope for transfer-audit; not attested by this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
    ClaimDefinition {
        claim_id: CLAIM_STRESS_TRANSMISSION,
        default_status: ClaimStatus::Unsupported,
        statement: "Stress transmission is out of scope for transfer-audit.",
        required_evidence_kinds: &[],
        limitations: &["Out of scope for transfer-audit; not attested by this toolkit."],
        warnings: &[],
        produced_by: "transfer-audit",
    },
];

pub fn catalog() -> &'static [ClaimDefinition] {
    CATALOG
}

pub fn lookup_claim(claim_id: &str) -> Option<&'static ClaimDefinition> {
    CATALOG.iter().find(|c| c.claim_id == claim_id)
}

pub fn all_catalog_claim_ids() -> Vec<&'static str> {
    CATALOG.iter().map(|c| c.claim_id).collect()
}

pub fn transfer_audit_supported_claim_ids() -> &'static [&'static str] {
    &[
        CLAIM_TRANSFER_ACTIVITY_RECONSTRUCTIBLE,
        CLAIM_SUPPLY_SNAPSHOT_AVAILABLE,
        CLAIM_SUPPLY_RECONCILIATION_AVAILABLE,
    ]
}

pub fn transfer_audit_unsupported_claim_ids() -> &'static [&'static str] {
    &[
        CLAIM_CIRCULATING_SUPPLY_NOT_VERIFIED,
        CLAIM_FIAT_RESERVE_NOT_VERIFIED,
        CLAIM_LIQUIDITY_EXPOSURE_NOT_MEASURED,
        CLAIM_PEG_STABILITY,
        CLAIM_REDEMPTION_CAPACITY,
        CLAIM_USER_GEOGRAPHY,
        CLAIM_HOLDER_IDENTITY,
        CLAIM_ACTUAL_SWAP_ROUTING,
        CLAIM_ISSUER_INTENT,
        CLAIM_STRESS_TRANSMISSION,
    ]
}

pub fn cross_chain_supported_claim_ids() -> &'static [&'static str] {
    &[
        CLAIM_CROSS_CHAIN_PER_DEPLOYMENT_COMPARISON,
        CLAIM_PER_CHAIN_TOTAL_SUPPLY_NOT_CIRCULATING,
    ]
}

pub fn cross_chain_unsupported_claim_ids() -> &'static [&'static str] {
    &[CLAIM_BRIDGE_BACKING_NOT_VERIFIED]
}

pub fn audit_plan_out_of_scope_ids() -> &'static [&'static str] {
    transfer_audit_unsupported_claim_ids()
}

fn kind_to_candidate_paths(kind: ArtifactKind) -> &'static [&'static str] {
    match kind {
        ArtifactKind::CanonicalTransfers => &["canonical_transfers.csv"],
        ArtifactKind::TransferLog => &["decoded_transfers.csv"],
        ArtifactKind::SupplySnapshots => &["supply_snapshots.csv"],
        ArtifactKind::SupplyAudit => &["supply_audit.csv"],
        ArtifactKind::Provenance => &["provenance.json"],
        ArtifactKind::QaReport => &["qa_report.json"],
        ArtifactKind::CrossChainSummary => &["cross_chain_summary.json", "cross_chain_summary.md"],
        _ => &[],
    }
}

fn resolve_evidence_paths(def: &ClaimDefinition, available_paths: &HashSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for kind in def.required_evidence_kinds {
        for candidate in kind_to_candidate_paths(*kind) {
            if available_paths.contains(candidate) && seen.insert(*candidate) {
                out.push((*candidate).to_string());
            }
        }
    }
    out
}

/// Build a manifest claim from the catalog plus run-specific evidence paths.
pub fn instantiate_claim(
    claim_id: &str,
    available_paths: &HashSet<&str>,
    extra_warnings: &[String],
) -> Option<ClaimBoundary> {
    let def = lookup_claim(claim_id)?;
    let evidence = resolve_evidence_paths(def, available_paths);
    let mut warnings: Vec<String> = def.warnings.iter().map(|s| (*s).to_string()).collect();
    for w in extra_warnings {
        if !warnings.contains(w) {
            warnings.push(w.clone());
        }
    }
    Some(ClaimBoundary::new(
        def.claim_id,
        def.default_status,
        def.statement,
        evidence,
        def.limitations.iter().map(|s| (*s).to_string()).collect(),
        warnings,
    ))
}

pub fn instantiate_claims(
    claim_ids: &[&str],
    available_paths: &HashSet<&str>,
) -> Vec<ClaimBoundary> {
    claim_ids
        .iter()
        .filter_map(|id| instantiate_claim(id, available_paths, &[]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_transfer_and_cross_chain_claim_ids() {
        let ids = all_catalog_claim_ids();
        for id in [
            CLAIM_TRANSFER_ACTIVITY_RECONSTRUCTIBLE,
            CLAIM_SUPPLY_SNAPSHOT_AVAILABLE,
            CLAIM_SUPPLY_RECONCILIATION_AVAILABLE,
            CLAIM_CROSS_CHAIN_PER_DEPLOYMENT_COMPARISON,
            CLAIM_PER_CHAIN_TOTAL_SUPPLY_NOT_CIRCULATING,
            CLAIM_CIRCULATING_SUPPLY_NOT_VERIFIED,
            CLAIM_BRIDGE_BACKING_NOT_VERIFIED,
            CLAIM_FIAT_RESERVE_NOT_VERIFIED,
            CLAIM_LIQUIDITY_EXPOSURE_NOT_MEASURED,
        ] {
            assert!(ids.contains(&id), "missing catalog entry {id}");
        }
    }

    #[test]
    fn transfer_audit_claims_use_catalog_statements() {
        let paths: HashSet<&str> = [
            "canonical_transfers.csv",
            "decoded_transfers.csv",
            "supply_snapshots.csv",
            "supply_audit.csv",
            "provenance.json",
            "qa_report.json",
        ]
        .into_iter()
        .collect();
        let supported = instantiate_claims(transfer_audit_supported_claim_ids(), &paths);
        assert_eq!(supported.len(), 3);
        assert!(supported
            .iter()
            .all(|c| !c.statement.is_empty() && !c.limitations.is_empty()));
        let unsupported = instantiate_claims(transfer_audit_unsupported_claim_ids(), &paths);
        assert!(unsupported
            .iter()
            .all(|c| c.status == ClaimStatus::Unsupported));
        assert!(unsupported
            .iter()
            .any(|c| c.claim == CLAIM_LIQUIDITY_EXPOSURE_NOT_MEASURED));
    }

    #[test]
    fn bridge_and_liquidity_claims_are_unsupported_only() {
        for id in [
            CLAIM_BRIDGE_BACKING_NOT_VERIFIED,
            CLAIM_LIQUIDITY_EXPOSURE_NOT_MEASURED,
        ] {
            let def = lookup_claim(id).unwrap();
            assert_eq!(def.default_status, ClaimStatus::Unsupported);
            assert!(def.required_evidence_kinds.is_empty());
        }
    }
}
