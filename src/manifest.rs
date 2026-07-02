//! The one owning type for a profile's `profile.yaml` manifest.
//!
//! Before this, ~6 modules each declared a private struct deserializing just the
//! slice of `profile.yaml` they needed (id/title, fact_families, flows,
//! review_map, exec), so "what is a profile?" had no single answer and a schema
//! change was a scavenger hunt. `ProfileManifest` is that answer; every consumer
//! loads it and reads the field it wants.
//!
//! (`extends` is deliberately NOT resolved here — `inherit::read_extends_strict`
//! keeps its own minimal, tolerant parse because it runs per chain-element and
//! must not fail a whole chain on an unrelated field.)

use crate::config::VerbSpec;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Deserialize, Default, Debug)]
pub struct ProfileManifest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    // These model the on-disk schema so this struct is the single answer to
    // "what is a profile", but aren't read *through* the manifest yet: `extends`
    // is parsed by `inherit::read_extends_strict` (a tolerant hot path), and
    // `description`/`languages` are handled by the web authoring surface.
    #[serde(default)]
    #[allow(dead_code)]
    pub description: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub extends: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub fact_families: Vec<FactFamily>,
    #[serde(default)]
    pub flows: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub review_map: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub exec: BTreeMap<String, VerbSpec>,
}

/// A declared fact family. Only `id` is read here (extraction rules live in
/// `extractors.yaml`); extra keys like `symbol:` are ignored.
#[derive(Deserialize, Default, Debug, Clone)]
pub struct FactFamily {
    #[serde(default)]
    pub id: String,
}

impl ProfileManifest {
    /// Load `<kn>/profiles/<id>/profile.yaml`. An absent file yields an empty
    /// manifest (many callers tolerate a manifest-less profile); a present but
    /// unparseable file is an error.
    pub fn load(kn: &Path, id: &str) -> Result<Self, String> {
        let p = kn.join("profiles").join(id).join("profile.yaml");
        match std::fs::read_to_string(&p) {
            Ok(raw) => serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("read {}: {e}", p.display())),
        }
    }

    /// The declared fact-family ids (what `fact <family>` validates against).
    pub fn fact_family_ids(&self) -> Vec<String> {
        self.fact_families.iter().map(|f| f.id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_full_manifest_and_tolerates_absent_file() {
        let kn = tempfile::tempdir().unwrap();
        let d = kn.path().join("profiles").join("p");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("profile.yaml"),
            "id: p\ntitle: P\nlanguages: [rust]\nfact_families:\n  - { id: command, symbol: true }\nflows:\n  bugfix: [code.recent]\nreview_map:\n  command: [architecture]\nexec:\n  build: cargo build\n",
        )
        .unwrap();
        let m = ProfileManifest::load(kn.path(), "p").unwrap();
        assert_eq!(m.id, "p");
        assert_eq!(m.languages, vec!["rust".to_string()]);
        assert_eq!(m.fact_family_ids(), vec!["command".to_string()]);
        assert_eq!(m.flows["bugfix"], vec!["code.recent".to_string()]);
        assert_eq!(m.review_map["command"], vec!["architecture".to_string()]);
        assert_eq!(m.exec["build"].commands(), vec!["cargo build".to_string()]);
        // absent profile.yaml → empty manifest, not an error
        let empty = ProfileManifest::load(kn.path(), "missing").unwrap();
        assert!(empty.id.is_empty() && empty.flows.is_empty());
    }
}
