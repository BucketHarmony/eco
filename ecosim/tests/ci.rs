//! The CI gate itself: `.github/workflows/ci.yml` and the justfile's `ci` recipe run the
//! required steps, in order. Both are scanned as text; the steps are recognised by their commands.

use std::fs;
use std::path::Path;

/// The required steps, each identified by a command it must contain.
const STEPS: [(&str, &str); 8] = [
    ("fmt", "cargo fmt --check"),
    ("clippy", "cargo clippy --all-targets -- -D warnings"),
    ("test (debug)", "cargo test\n"),
    ("coverage", "cargo llvm-cov --fail-under-lines 85 --ignore-filename-regex 'main\\.rs|cli/'"),
    ("build release", "cargo build --release"),
    ("runs and checks", "--seed \"$s\" --ticks 20000"),
    ("sweep baseline", "sweep --baseline --seeds 1,2,3"),
    ("long run", "check --long ci-runs/long-s1"),
];

/// Index of the first block containing each step's command; panics naming a missing step.
fn positions(blocks: &[String], what: &str) -> Vec<usize> {
    STEPS
        .iter()
        .map(|(name, cmd)| {
            blocks.iter().position(|b| b.contains(cmd)).unwrap_or_else(|| panic!("{what}: step '{name}' missing"))
        })
        .collect()
}

fn assert_in_order(pos: &[usize], what: &str) {
    for (w, names) in pos.windows(2).zip(STEPS.windows(2)) {
        assert!(w[0] < w[1], "{what}: '{}' must come before '{}'", names[0].0, names[1].0);
    }
}

/// Text of each block, split before every line matching `starts`; each block ends with a newline.
fn blocks(text: &str, starts: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out = vec![String::new()];
    for line in text.lines() {
        if starts(line) {
            out.push(String::new());
        }
        let b = out.last_mut().unwrap();
        b.push_str(line.trim());
        b.push('\n');
    }
    out
}

#[test]
fn ci_workflow_runs_the_required_steps_in_order() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/ci.yml");
    let yml = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let steps = blocks(&yml, |l| l.trim_start().starts_with("- "));
    let pos = positions(&steps, "ci.yml");
    assert_in_order(&pos, "ci.yml");
    for artifact in ["ecosim/lcov.info", "ecosim/ci-runs/*/series.csv", "ecosim/ci-runs/baseline-margins.txt"] {
        assert!(yml.contains(&format!("path: {artifact}")), "ci.yml: no upload of {artifact}");
    }
    assert!(yml.contains("ECOSIM_REQUIRE_CROSS=1 cargo test --release"), "ci.yml: no release determinism step");
    assert!(yml.contains("hashFiles('ecosim/Cargo.lock')"), "ci.yml: cache not keyed on Cargo.lock");
    assert!(
        yml.contains(
            "run: cargo bench --bench tick
"
        ),
        "ci.yml: no tick bench step"
    );
    assert!(yml.contains("path: ecosim/ci-runs/bench-tick.json"), "ci.yml: no upload of the bench numbers");
}

#[test]
fn justfile_ci_recipe_runs_the_same_steps_in_the_same_order() {
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("justfile")).unwrap();
    let recipes = blocks(&text, |l| !l.starts_with(' ') && !l.starts_with('#') && l.contains(':') && !l.contains(":="));
    let ci = recipes.iter().find(|r| r.starts_with("ci:")).expect("justfile: no ci recipe");
    let deps: Vec<&str> = ci.lines().next().unwrap()["ci:".len()..].split_whitespace().collect();
    let ordered: Vec<String> = deps
        .iter()
        .map(|d| {
            recipes.iter().find(|r| r.starts_with(&format!("{d}:"))).unwrap_or_else(|| panic!("no recipe {d}")).clone()
        })
        .collect();
    let pos = positions(&ordered, "justfile ci");
    assert_in_order(&pos, "justfile ci");
}
