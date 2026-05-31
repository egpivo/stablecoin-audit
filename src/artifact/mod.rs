pub mod manifest;
pub mod writer;

pub use manifest::{
    ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef, ClaimBoundary, ClaimStatus,
    InputRef, SourceSnapshot, SCHEMA,
};
pub use writer::{
    resolve_artifact_under_root, validate_manifest_paths, validate_relative_artifact_path,
    write_artifact_manifest, write_manifest, MANIFEST_FILENAME,
};
