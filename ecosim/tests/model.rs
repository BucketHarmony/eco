//! `MODEL.md` documents every parameter (shot 26).
//!
//! The model reference is only worth reading if it is complete, and nothing else holds it complete: a
//! shot that adds a key to `params.toml` without a line in `MODEL.md` fails here. A key counts as
//! documented when its full dotted name appears in backticks, `` `grass.npk.need_n` `` -- the form
//! every rule's parameter list uses, and one a stray word in the prose cannot satisfy by accident.
//!
//! The keys come from two places, and both are checked: the text of `params.toml`, and the loaded
//! `Params` serialized back out, which is what `meta.json` carries. The second catches a field with a
//! serde default that the file leaves out.

use ecosim::Params;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Every leaf key under `v`, as a dotted path. An array is a leaf: a curve or a triple is one key.
fn leaves(prefix: &str, v: &toml::Value, out: &mut BTreeSet<String>) {
    match v {
        toml::Value::Table(t) => {
            for (k, x) in t {
                let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                leaves(&path, x, out);
            }
        }
        _ => {
            out.insert(prefix.to_string());
        }
    }
}

/// The keys of `keys` that `doc` does not name in backticks.
fn undocumented<'a>(keys: &'a BTreeSet<String>, doc: &str) -> Vec<&'a str> {
    keys.iter().filter(|k| !doc.contains(&format!("`{k}`"))).map(String::as_str).collect()
}

fn read(name: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(name)).unwrap()
}

/// The keys written in `params.toml`, and the keys of the loaded `Params`.
fn param_keys() -> (BTreeSet<String>, BTreeSet<String>) {
    let file: toml::Value = toml::Value::Table(read("params.toml").parse::<toml::Table>().unwrap());
    let loaded = toml::Value::try_from(Params::load_default()).unwrap();
    let (mut a, mut b) = (BTreeSet::new(), BTreeSet::new());
    leaves("", &file, &mut a);
    leaves("", &loaded, &mut b);
    (a, b)
}

/// Acceptance: every key in `params.toml`, and every key `Params` has, is named in `MODEL.md`.
#[test]
fn every_params_key_is_documented_in_model_md() {
    let doc = read("MODEL.md");
    let (file, loaded) = param_keys();
    // Sanity: the walk found the file, the nested tables and the arrays, not an empty set.
    assert!(file.len() > 200, "only {} keys found in params.toml", file.len());
    for k in ["medium.roof.plantable", "grass.npk.need_n", "tree.light", "npk.init_detritus"] {
        assert!(file.contains(k), "the walk missed {k}");
    }
    let missing = undocumented(&file, &doc);
    assert!(missing.is_empty(), "{} params.toml keys are not in MODEL.md: {missing:?}", missing.len());
    let missing = undocumented(&loaded, &doc);
    assert!(missing.is_empty(), "{} Params keys are not in MODEL.md: {missing:?}", missing.len());
}

/// Regression sibling: the check is not vacuous. Dropping one key's mention from the document, or
/// naming it without backticks, is caught; and a key named only as a prefix of a longer one does
/// not count (`tree.npk` is not `tree.npk.need_n`).
#[test]
fn a_key_missing_from_model_md_is_reported() {
    let doc = read("MODEL.md");
    let (file, _) = param_keys();
    let k = "tree.npk.need_n";
    let without = doc.replace(&format!("`{k}`"), k);
    assert_eq!(undocumented(&file, &without), vec![k]);
    let one: BTreeSet<String> = [k.to_string()].into();
    assert_eq!(undocumented(&one, "see `tree.npk` and `tree.npk.need_n.x`"), vec![k]);
}
