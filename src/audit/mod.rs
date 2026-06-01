pub mod canonical;
pub mod claims;
pub mod contracts;
pub mod supply;

pub use canonical::{write_canonical_audit_tables, CanonicalWriteParams};
pub use claims::{
    all_catalog_claim_ids, audit_plan_out_of_scope_ids, cross_chain_supported_claim_ids,
    cross_chain_unsupported_claim_ids, instantiate_claim, instantiate_claims, lookup_claim,
    transfer_audit_supported_claim_ids, transfer_audit_unsupported_claim_ids, ClaimDefinition,
    CLAIM_BRIDGE_BACKING_NOT_VERIFIED, CLAIM_CIRCULATING_SUPPLY_NOT_VERIFIED,
    CLAIM_CROSS_CHAIN_PER_DEPLOYMENT_COMPARISON, CLAIM_FIAT_RESERVE_NOT_VERIFIED,
    CLAIM_LIQUIDITY_EXPOSURE_NOT_MEASURED, CLAIM_PER_CHAIN_TOTAL_SUPPLY_NOT_CIRCULATING,
    CLAIM_SUPPLY_RECONCILIATION_AVAILABLE, CLAIM_SUPPLY_SNAPSHOT_AVAILABLE,
    CLAIM_TRANSFER_ACTIVITY_RECONSTRUCTIBLE,
};
pub use contracts::{
    BlockRange, CanonicalTransferRecord, ChainWindowEntry, ChainWindowsDocument, DeploymentEntry,
    DeploymentRegistry, DeploymentRole, EvidenceSource, EvidenceSourcesDocument, SourceType,
    SupplySnapshotRecord, TimestampRange, TransferEventType, CANONICAL_TRANSFERS_FILENAME,
    CANONICAL_TRANSFERS_SCHEMA, CHAIN_WINDOWS_FILENAME, CHAIN_WINDOWS_SCHEMA,
    DEPLOYMENT_REGISTRY_FILENAME, DEPLOYMENT_REGISTRY_SCHEMA, EVIDENCE_SOURCES_FILENAME,
    EVIDENCE_SOURCES_SCHEMA, SUPPLY_SNAPSHOTS_FILENAME, SUPPLY_SNAPSHOTS_SCHEMA,
};
pub use supply::{
    build_supply_metrics_from_events, compute_supply_invariant, SupplyAuditRow, SupplyMetrics,
};
