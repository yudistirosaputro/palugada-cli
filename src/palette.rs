//! Flattens pickable convention sections (with bodies + provenance) across the
//! active profile + its `extends` chain, and lists other profiles for lazy expand.
//!
//! Upstream signatures confirmed (src/inherit.rs, src/knowledge.rs) before
//! implementing:
//! - `inherit::MergedSection` field for the section id is `anchor` (inherit.rs:77).
//! - `inherit::resolve_convention_raw` returns markdown that STILL INCLUDES the
//!   `---` front-matter (either the verbatim child .md, or `render_merged`'s
//!   output which re-adds a synthesized front-matter block) — so it must be
//!   stripped via `knowledge::strip_frontmatter` before `inherit::parse_sections`.
//! - `inherit::parse_sections(body: &str) -> Vec<MergedSection>` — takes `&str`.
//! - `knowledge::strip_frontmatter` is already `pub` (not private) — no
//!   visibility change needed.
//! - `knowledge::conventions(kn, profile) -> Result<Vec<TopicMeta>, String>`;
//!   `TopicMeta.id` and `SectionMeta.id/.title/.tokens/.origin/.from` all exist
//!   as plain `String`/`usize` fields (knowledge.rs:171-207).
//! - `knowledge::convention_md(kn, profile, id) -> Result<String, String>` exists.
//! - `profile::list(kn) -> Result<Vec<(String, String)>, String>` returns
//!   `(id, title)` pairs — matches the brief.
//!
//! This module (Task B1) is not yet wired to any caller — the HTTP routes
//! that consume `palette()`/`profile_sections()` land in Task B2 (`src/web.rs`).
//! `#[allow(dead_code)]` below is intentional and temporary until that wiring lands.
#![allow(dead_code)]
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Default)]
pub struct PaletteSection {
    pub source_profile: String,
    pub topic_id: String,
    pub section_id: String,
    pub title: String,
    pub tokens: usize,
    pub origin: String, // own | overridden | inherited | other-profile
    pub from: String,
    pub body: String,
}

#[derive(Serialize, Default)]
pub struct Palette {
    pub active_profile: String,
    pub chain: Vec<String>,
    pub sections: Vec<PaletteSection>,
    pub other_profiles: Vec<String>,
}

/// Pickable sections across `active`'s profile + its `extends` chain, plus the
/// ids of other on-disk profiles (outside the chain) for lazy expansion.
pub fn palette(kn: &Path, active: &str) -> Result<Palette, String> {
    let chain = crate::inherit::resolve_chain(kn, active)?;
    let topics = crate::inherit::merged_conventions_provenance(kn, active)?;
    let mut sections = Vec::new();
    for t in &topics {
        // Merged body across the chain (front-matter + preamble + ## sections).
        let raw = crate::inherit::resolve_convention_raw(kn, active, &t.id)?;
        let parsed = match raw {
            Some(md) => crate::inherit::parse_sections(crate::knowledge::strip_frontmatter(&md)),
            None => Vec::new(),
        };
        for sm in &t.sections {
            match parsed.iter().find(|ps| ps.anchor == sm.id) {
                Some(ps) => sections.push(PaletteSection {
                    source_profile: active.to_string(),
                    topic_id: t.id.clone(),
                    section_id: sm.id.clone(),
                    title: sm.title.clone(),
                    tokens: sm.tokens,
                    origin: if sm.origin.is_empty() { "own".into() } else { sm.origin.clone() },
                    from: if sm.from.is_empty() { active.to_string() } else { sm.from.clone() },
                    body: ps.body.clone(),
                }),
                // md<->index id drift: skip-with-warning, never crash.
                None => eprintln!("palette: no body for {}#{} (md/index drift)", t.id, sm.id),
            }
        }
    }
    let all = crate::profile::list(kn)?;
    let other_profiles = all.into_iter().map(|(id, _)| id).filter(|id| !chain.contains(id)).collect();
    Ok(Palette { active_profile: active.to_string(), chain, sections, other_profiles })
}

/// One profile's OWN convention sections (with bodies), for lazy "other profile" expansion.
pub fn profile_sections(kn: &Path, profile: &str) -> Result<Vec<PaletteSection>, String> {
    let mut out = Vec::new();
    for t in crate::knowledge::conventions(kn, profile)? {
        let md = crate::knowledge::convention_md(kn, profile, &t.id)?;
        for ps in crate::inherit::parse_sections(crate::knowledge::strip_frontmatter(&md)) {
            out.push(PaletteSection {
                source_profile: profile.to_string(),
                topic_id: t.id.clone(),
                section_id: ps.anchor.clone(),
                title: ps.title.clone(),
                tokens: ps.body.len() / 4 + 8,
                origin: "other-profile".into(),
                from: profile.to_string(),
                body: ps.body.clone(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Build a throwaway knowledge dir with one profile + one convention.
    fn seed(kn: &std::path::Path, profile: &str, conv_id: &str) {
        let cdir = kn.join("profiles").join(profile).join("conventions");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(kn.join("profiles").join(profile).join("profile.yaml"),
            format!("id: {profile}\ntitle: {profile}\nlanguages: [rust]\n")).unwrap();
        fs::write(cdir.join(format!("{conv_id}.md")),
            "---\nid: errh\ntitle: Error handling\ndescription: d\ntags: [e]\n---\n\n# Error handling\n\n## Modeling failures {#modeling-failures}\nModel errors explicitly.\n").unwrap();
        fs::write(cdir.join("_index.json"),
            "{\"schema_version\":\"1.0\",\"topics\":[{\"id\":\"errh\",\"title\":\"Error handling\",\"description\":\"d\",\"file\":\"errh.md\",\"tags\":[\"e\"],\"sections\":[{\"id\":\"modeling-failures\",\"title\":\"Modeling failures\",\"tokens\":40}]}]}").unwrap();
    }

    #[test]
    fn palette_returns_own_section_with_body_and_origin() {
        let tmp = std::env::temp_dir().join(format!("pal-{}", std::process::id()));
        let kn = tmp.join("kn");
        seed(&kn, "rust-cli", "errh");
        let p = palette(&kn, "rust-cli").unwrap();
        let s = p.sections.iter().find(|s| s.section_id == "modeling-failures").expect("section present");
        assert_eq!(s.topic_id, "errh");
        assert_eq!(s.origin, "own");
        assert_eq!(s.from, "rust-cli");
        assert!(s.body.contains("Model errors explicitly"), "body copied from .md");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn palette_lists_other_profiles_not_in_chain() {
        let tmp = std::env::temp_dir().join(format!("pal2-{}", std::process::id()));
        let kn = tmp.join("kn");
        seed(&kn, "rust-cli", "errh");
        seed(&kn, "flutter-bloc", "errh");
        let p = palette(&kn, "rust-cli").unwrap();
        assert!(p.other_profiles.contains(&"flutter-bloc".to_string()));
        assert!(!p.other_profiles.contains(&"rust-cli".to_string()));
        let _ = fs::remove_dir_all(&tmp);
    }
}
