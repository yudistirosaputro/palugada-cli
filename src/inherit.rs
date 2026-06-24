//! Profile inheritance (`extends`): resolve a child profile's `extends` chain
//! and merge inherited conventions/recipes. A profile with no `extends` has a
//! chain of `[self]`, so every resolver here is a no-op for flat profiles.

use std::collections::BTreeSet;
use std::path::Path;

/// Maximum length of an `extends` chain (cycle/runaway backstop).
pub const MAX_DEPTH: usize = 8;

/// Read just the `extends:` scalar from `<id>/profile.yaml` (None if absent,
/// empty, unreadable, or unparseable — a malformed parent simply ends the chain
/// and is surfaced separately by `profile validate`).
#[allow(dead_code)] // temporary: consumed by Task 2 merge logic
pub fn read_extends(kn: &Path, id: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct E {
        #[serde(default)]
        extends: Option<String>,
    }
    let p = kn.join("profiles").join(id).join("profile.yaml");
    let raw = std::fs::read_to_string(&p).ok()?;
    let e: E = serde_yaml::from_str(&raw).ok()?;
    e.extends.filter(|s| !s.is_empty())
}

/// The `extends` chain for `id`, most-derived first: `[id, parent, ...]`.
/// Errors on a missing profile in the chain, a cycle, or a chain deeper than
/// `MAX_DEPTH`.
#[allow(dead_code)] // temporary: consumed by Task 2 merge logic
pub fn resolve_chain(kn: &Path, id: &str) -> Result<Vec<String>, String> {
    let mut chain: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cur = id.to_string();
    loop {
        if !seen.insert(cur.clone()) {
            let mut path = chain.clone();
            path.push(cur.clone());
            return Err(format!("inheritance cycle: {}", path.join(" \u{2192} ")));
        }
        if !kn
            .join("profiles")
            .join(&cur)
            .join("profile.yaml")
            .is_file()
        {
            return match chain.last() {
                Some(child) => Err(format!(
                    "profile '{child}' extends '{cur}' which does not exist"
                )),
                None => Err(format!(
                    "profile '{cur}' has no profile.yaml at {}",
                    kn.join("profiles").join(&cur).display()
                )),
            };
        }
        chain.push(cur.clone());
        if chain.len() > MAX_DEPTH {
            return Err(format!(
                "inheritance chain too deep (> {MAX_DEPTH}) starting at '{id}'"
            ));
        }
        match read_extends(kn, &cur) {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    Ok(chain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Write a minimal `profile.yaml` for `id` with an optional `extends`.
    fn profile(kn: &Path, id: &str, extends: Option<&str>) {
        let dir = kn.join("profiles").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let mut y = format!("id: {id}\n");
        if let Some(e) = extends {
            y.push_str(&format!("extends: {e}\n"));
        }
        std::fs::write(dir.join("profile.yaml"), y).unwrap();
    }

    #[test]
    fn chain_for_flat_profile_is_self_only() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "rust-cli", None);
        assert_eq!(
            resolve_chain(kn.path(), "rust-cli").unwrap(),
            vec!["rust-cli".to_string()]
        );
        assert_eq!(read_extends(kn.path(), "rust-cli"), None);
    }

    #[test]
    fn chain_walks_parents_most_derived_first() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-base", None);
        profile(kn.path(), "android-mvvm", Some("android-base"));
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        assert_eq!(
            resolve_chain(kn.path(), "android-mvi").unwrap(),
            vec![
                "android-mvi".to_string(),
                "android-mvvm".to_string(),
                "android-base".to_string()
            ]
        );
    }

    #[test]
    fn missing_parent_errors_with_child_name() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvi", Some("android-foo"));
        let err = resolve_chain(kn.path(), "android-mvi").unwrap_err();
        assert!(
            err.contains("android-mvi") && err.contains("android-foo"),
            "{err}"
        );
    }

    #[test]
    fn cycle_is_detected() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "a", Some("b"));
        profile(kn.path(), "b", Some("a"));
        let err = resolve_chain(kn.path(), "a").unwrap_err();
        assert!(err.contains("cycle"), "{err}");
    }

    #[test]
    fn over_max_depth_errors() {
        let kn = tempfile::tempdir().unwrap();
        // p0 (root) <- p1 <- ... <- p9  => chain of 10 > MAX_DEPTH (8)
        profile(kn.path(), "p0", None);
        for i in 1..=9 {
            profile(kn.path(), &format!("p{i}"), Some(&format!("p{}", i - 1)));
        }
        let err = resolve_chain(kn.path(), "p9").unwrap_err();
        assert!(err.contains("too deep"), "{err}");
    }
}
