//! Cargo feature resolution over the workspace's own manifests.
//!
//! `cargo metadata`'s resolved feature sets are unified across the whole
//! workspace, so they cannot say what a *standalone* build of one crate
//! enables. This module recomputes that from the manifests in
//! `cargo metadata --no-deps`: starting at one package with its default
//! features, it follows normal and build dependencies (not dev-dependencies,
//! which `cargo build -p` does not build) between workspace members, and
//! expands feature tables, `dep:` entries, `dep/feature` and `dep?/feature`
//! entries and implicit optional-dependency features, to a fixed point.
//!
//! Only workspace members are followed: an external crate cannot enable a
//! feature of a `dive-*` crate.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The features a standalone build of `root` enables, per workspace package.
/// A package that the build does not reach has no entry.
pub fn standalone_features(packages: &[Value], root: &str) -> BTreeMap<String, BTreeSet<String>> {
    let by_name: BTreeMap<&str, &Value> = packages
        .iter()
        .filter_map(|p| p["name"].as_str().map(|n| (n, p)))
        .collect();
    let mut feats: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut active_deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if !by_name.contains_key(root) {
        return feats;
    }
    feats
        .entry(root.to_owned())
        .or_default()
        .insert("default".to_owned());
    loop {
        let before = (feats.clone(), active_deps.clone());
        let active: Vec<String> = feats.keys().cloned().collect();
        for pkg in active {
            let Some(&p) = by_name.get(pkg.as_str()) else {
                continue;
            };
            let deps: Vec<&Value> = p["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| d["kind"].as_str() != Some("dev"))
                .collect();
            let key = |d: &Value| -> String {
                d["rename"]
                    .as_str()
                    .or_else(|| d["name"].as_str())
                    .unwrap_or_default()
                    .to_owned()
            };
            let optional_keys: BTreeSet<String> = deps
                .iter()
                .filter(|d| d["optional"].as_bool() == Some(true))
                .map(|d| key(d))
                .collect();
            let mut on_deps = active_deps.get(&pkg).cloned().unwrap_or_default();
            for d in &deps {
                if d["optional"].as_bool() != Some(true) {
                    on_deps.insert(key(d));
                }
            }
            // Expand this package's enabled features.
            let mut on_feats = feats.get(&pkg).cloned().unwrap_or_default();
            let mut dep_feats: Vec<(String, String)> = Vec::new();
            let table = &p["features"];
            let mut queue: Vec<String> = on_feats.iter().cloned().collect();
            while let Some(f) = queue.pop() {
                let entries = table[f.as_str()].as_array();
                if entries.is_none() && optional_keys.contains(&f) {
                    on_deps.insert(f.clone());
                }
                for e in entries.into_iter().flatten().filter_map(Value::as_str) {
                    if let Some(dep) = e.strip_prefix("dep:") {
                        on_deps.insert(dep.to_owned());
                    } else if let Some((dep, feat)) = e.split_once('/') {
                        if let Some(weak) = dep.strip_suffix('?') {
                            dep_feats.push((weak.to_owned(), feat.to_owned()));
                        } else {
                            on_deps.insert(dep.to_owned());
                            dep_feats.push((dep.to_owned(), feat.to_owned()));
                        }
                    } else if on_feats.insert(e.to_owned()) {
                        queue.push(e.to_owned());
                    }
                }
            }
            // Activate dependencies and pass features down.
            for d in &deps {
                let k = key(d);
                if !on_deps.contains(&k) {
                    continue;
                }
                let Some(target) = d["name"].as_str() else {
                    continue;
                };
                if !by_name.contains_key(target) {
                    continue;
                }
                let t = feats.entry(target.to_owned()).or_default();
                if d["uses_default_features"].as_bool() != Some(false) {
                    t.insert("default".to_owned());
                }
                for f in d["features"].as_array().into_iter().flatten() {
                    if let Some(f) = f.as_str() {
                        t.insert(f.to_owned());
                    }
                }
                for (_, feat) in dep_feats.iter().filter(|(dep, _)| *dep == k) {
                    t.insert(feat.clone());
                }
            }
            feats.insert(pkg.clone(), on_feats);
            active_deps.insert(pkg, on_deps);
        }
        if (feats.clone(), active_deps.clone()) == before {
            return feats;
        }
    }
}

/// Whether a standalone build of `root` enables `feature` of `package`.
pub fn enables(packages: &[Value], root: &str, package: &str, feature: &str) -> bool {
    standalone_features(packages, root)
        .get(package)
        .is_some_and(|f| f.contains(feature))
}

#[cfg(test)]
mod tests {
    use super::enables;
    use serde_json::{Value, json};

    fn dep(name: &str, default: bool, features: &[&str]) -> Value {
        json!({"name": name, "rename": null, "kind": null, "optional": false,
               "uses_default_features": default, "features": features})
    }

    fn eval() -> Value {
        json!({"name": "dive-eval", "dependencies": [],
               "features": {"default": ["enumerate"], "enumerate": []}})
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "test fixtures read better built inline"
    )]
    fn pkg(name: &str, deps: Vec<Value>, features: Value) -> Value {
        json!({"name": name, "dependencies": deps, "features": features})
    }

    fn violates(pkgs: &[Value]) -> bool {
        enables(pkgs, "dive-trace", "dive-eval", "enumerate")
    }

    #[test]
    fn default_features_enable_the_enumerator() {
        let pkgs = [
            eval(),
            pkg("dive-trace", vec![dep("dive-eval", true, &[])], json!({})),
        ];
        assert!(violates(&pkgs));
    }

    #[test]
    fn default_features_off_is_clean() {
        let pkgs = [
            eval(),
            pkg("dive-trace", vec![dep("dive-eval", false, &[])], json!({})),
        ];
        assert!(!violates(&pkgs));
    }

    #[test]
    fn explicit_feature_is_caught() {
        let pkgs = [
            eval(),
            pkg(
                "dive-trace",
                vec![dep("dive-eval", false, &["enumerate"])],
                json!({}),
            ),
        ];
        assert!(violates(&pkgs));
    }

    #[test]
    fn transitive_through_model_is_caught() {
        let pkgs = [
            eval(),
            pkg("dive-model", vec![dep("dive-eval", true, &[])], json!({})),
            pkg("dive-trace", vec![dep("dive-model", false, &[])], json!({})),
        ];
        assert!(violates(&pkgs));
    }

    #[test]
    fn feature_of_an_intermediate_crate_is_caught() {
        let pkgs = [
            eval(),
            pkg(
                "dive-model",
                vec![dep("dive-eval", false, &[])],
                json!({"default": ["fast"], "fast": ["dive-eval/enumerate"]}),
            ),
            pkg("dive-trace", vec![dep("dive-model", true, &[])], json!({})),
        ];
        assert!(violates(&pkgs));
        // The same crate with its default features off is clean.
        let pkgs = [
            eval(),
            pkg(
                "dive-model",
                vec![dep("dive-eval", false, &[])],
                json!({"default": ["fast"], "fast": ["dive-eval/enumerate"]}),
            ),
            pkg("dive-trace", vec![dep("dive-model", false, &[])], json!({})),
        ];
        assert!(!violates(&pkgs));
    }

    #[test]
    fn root_feature_table_is_caught() {
        let pkgs = [
            eval(),
            pkg(
                "dive-trace",
                vec![dep("dive-eval", false, &[])],
                json!({"default": ["x"], "x": ["dive-eval/enumerate"]}),
            ),
        ];
        assert!(violates(&pkgs));
    }

    #[test]
    fn inactive_optional_and_dev_dependencies_do_not_count() {
        let mut optional = dep("dive-eval", true, &[]);
        optional["optional"] = json!(true);
        let mut dev = dep("dive-eval", true, &[]);
        dev["kind"] = json!("dev");
        let pkgs = [
            eval(),
            pkg(
                "dive-trace",
                vec![optional.clone(), dev],
                json!({"extra": ["dep:dive-eval"]}),
            ),
        ];
        assert!(!violates(&pkgs));
        // Turning the optional dependency on by default does count.
        let pkgs = [
            eval(),
            pkg(
                "dive-trace",
                vec![optional],
                json!({"default": ["extra"], "extra": ["dep:dive-eval"]}),
            ),
        ];
        assert!(violates(&pkgs));
    }

    #[test]
    fn weak_dependency_features_need_the_dependency() {
        let mut optional = dep("dive-eval", false, &[]);
        optional["optional"] = json!(true);
        let pkgs = [
            eval(),
            pkg(
                "dive-trace",
                vec![optional.clone()],
                json!({"default": ["w"], "w": ["dive-eval?/enumerate"]}),
            ),
        ];
        assert!(!violates(&pkgs));
        let pkgs = [
            eval(),
            pkg(
                "dive-trace",
                vec![optional],
                json!({"default": ["w", "dive-eval"], "w": ["dive-eval?/enumerate"]}),
            ),
        ];
        assert!(violates(&pkgs));
    }

    #[test]
    fn renamed_dependencies_resolve_by_package_name() {
        let mut renamed = dep("dive-eval", false, &[]);
        renamed["rename"] = json!("ev");
        let pkgs = [
            eval(),
            pkg(
                "dive-trace",
                vec![renamed],
                json!({"default": ["x"], "x": ["ev/enumerate"]}),
            ),
        ];
        assert!(violates(&pkgs));
    }
}
