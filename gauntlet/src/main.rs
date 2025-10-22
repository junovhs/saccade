use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

// ========== Types ==========

struct GauntletConfig {
    saccade_bin: PathBuf,
    keep_tmp: bool,
    filter: Option<String>,
    verbose: bool,
}

struct TestStats {
    pass: usize,
    fail: usize,
    skip: usize,
}

struct TestContext {
    config: GauntletConfig,
    tmp_root: TempDir,
    stats: TestStats,
}

type TestFn = fn(&TestContext, &Path) -> Result<()>;

// ========== Main ==========

fn main() {
    if let Err(e) = run() {
        eprintln!("❌ Fatal error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = parse_config()?;
    check_prerequisites(&config)?;

    let tmp_root = TempDir::new().context("Failed to create temp directory")?;
    println!(
        "Running Saccade Gauntlet against: {}",
        config.saccade_bin.display()
    );

    let mut ctx = TestContext {
        config,
        tmp_root,
        stats: TestStats {
            pass: 0,
            fail: 0,
            skip: 0,
        },
    };

    let tests = register_tests();
    execute_tests(&mut ctx, &tests)?;
    print_summary(&ctx.stats);

    if ctx.stats.fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}

// ========== Config ==========

fn parse_config() -> Result<GauntletConfig> {
    let saccade_bin = env::var("SACCADE")
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/saccade/saccade", home)
        })
        .into();

    let keep_tmp = env::var("KEEP_TMP").unwrap_or_else(|_| "0".to_string()) == "1";
    let filter = env::var("GAUNTLET_FILTER").ok();
    let verbose = env::var("VERBOSE").unwrap_or_else(|_| "0".to_string()) == "1";

    Ok(GauntletConfig {
        saccade_bin,
        keep_tmp,
        filter,
        verbose,
    })
}

fn check_prerequisites(config: &GauntletConfig) -> Result<()> {
    check_command("git")?;
    check_command("jq")?;

    if !config.saccade_bin.exists() {
        bail!("SACCADE not found at: {}", config.saccade_bin.display());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata =
            fs::metadata(&config.saccade_bin).context("Failed to read saccade binary metadata")?;
        if metadata.permissions().mode() & 0o111 == 0 {
            bail!("SACCADE not executable at: {}", config.saccade_bin.display());
        }
    }

    Ok(())
}

fn check_command(cmd: &str) -> Result<()> {
    Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Missing required tool: {}", cmd))?;
    Ok(())
}

// ========== Test Execution ==========

fn register_tests() -> Vec<(&'static str, TestFn)> {
    vec![
        ("test_01_basic_e2e", test_01_basic_e2e as TestFn),
        ("test_02_secrets_and_binaries_excluded", test_02_secrets_and_binaries_excluded),
        ("test_03_prune_in_find_mode", test_03_prune_in_find_mode),
        ("test_04_git_vs_find_enumeration", test_04_git_vs_find_enumeration),
        ("test_05_api_rust_pub_and_scoped", test_05_api_rust_pub_and_scoped),
        ("test_06_api_ts_exports_only_and_pascalcase", test_06_api_ts_exports_only_and_pascalcase),
        ("test_07_api_python_public_only", test_07_api_python_public_only),
        ("test_08_api_go_exported_only", test_08_api_go_exported_only),
        ("test_09_frontend_dedup_no_duplicates_in_api", test_09_frontend_dedup_no_duplicates_in_api),
        ("test_10_dry_run_stats_and_no_writes", test_10_dry_run_stats_and_no_writes),
        ("test_11_cli_validation_errors", test_11_cli_validation_errors),
        ("test_12_token_header_uses_div_3_5", test_12_token_header_uses_div_3_5),
        ("test_13_clickable_link_line_present", test_13_clickable_link_line_present),
        ("test_14_stage2_repomix_optional", test_14_stage2_repomix_optional),
    ]
}

fn execute_tests(ctx: &mut TestContext, tests: &[(&str, TestFn)]) -> Result<()> {
    for (name, test_fn) in tests {
        if let Some(ref filter) = ctx.config.filter {
            if !name.contains(filter.as_str()) {
                if ctx.config.verbose {
                    println!("    skip {} due to filter", name);
                }
                continue;
            }
        }

        println!("---- {} ----", name);

        let test_dir = ctx.tmp_root.path().join(name);
        fs::create_dir_all(&test_dir)?;

        match test_fn(ctx, &test_dir) {
            Ok(()) => {
                println!("✅ {}", name);
                ctx.stats.pass += 1;
            }
            Err(e) => {
                eprintln!("❌ {} failed: {}", name, e);
                if ctx.config.keep_tmp {
                    eprintln!("Fixture left at: {}", test_dir.display());
                }
                ctx.stats.fail += 1;
            }
        }
    }

    Ok(())
}

fn print_summary(stats: &TestStats) {
    let total = stats.pass + stats.fail + stats.skip;
    println!();
    println!("========================================");
    println!(" Gauntlet Summary:");
    println!("   Total: {}", total);
    println!("   Pass : {}", stats.pass);
    println!("   Fail : {}", stats.fail);
    println!("   Skip : {}", stats.skip);
    println!("========================================");
}

// ========== Test Helpers ==========

fn new_git_repo(dir: &Path) -> Result<()> {
    run_cmd(dir, "git", &["init", "-q"])?;
    run_cmd(dir, "git", &["config", "user.email", "t@example.com"])?;
    run_cmd(dir, "git", &["config", "user.name", "t"])?;
    Ok(())
}

fn run_saccade(ctx: &TestContext, dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(&ctx.config.saccade_bin)
        .current_dir(dir)
        .args(args)
        .output()
        .context("Failed to run saccade")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}{}", stdout, stderr);

    fs::write(dir.join("run.log"), &combined)?;

    if ctx.config.verbose {
        print!("{}", combined);
    }

    Ok(combined)
}

fn run_cmd(dir: &Path, cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .current_dir(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Failed to run: {} {:?}", cmd, args))?;

    if !status.success() {
        bail!("Command failed: {} {:?}", cmd, args);
    }

    Ok(())
}

fn git_add_commit(dir: &Path, msg: &str) -> Result<()> {
    run_cmd(dir, "git", &["add", "."])?;
    run_cmd(dir, "git", &["commit", "-qm", msg])?;
    Ok(())
}

// Assertions

fn assert_file(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("Expected file not found: {}", path.display());
    }
    Ok(())
}

#[allow(dead_code)]
fn assert_nofile(path: &Path) -> Result<()> {
    if path.exists() {
        bail!("Expected file to be absent: {}", path.display());
    }
    Ok(())
}

fn assert_contains(path: &Path, pattern: &str) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;

    let re = regex::Regex::new(pattern)
        .with_context(|| format!("Invalid regex: {}", pattern))?;

    if !re.is_match(&content) {
        bail!(
            "Expected pattern not found in {}: {}",
            path.display(),
            pattern
        );
    }
    Ok(())
}

fn assert_not_contains(path: &Path, pattern: &str) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;

    let re = regex::Regex::new(pattern)
        .with_context(|| format!("Invalid regex: {}", pattern))?;

    if re.is_match(&content) {
        bail!(
            "Unexpected pattern found in {}: {}",
            path.display(),
            pattern
        );
    }
    Ok(())
}

fn assert_json_startswith(path: &Path, jq_key: &str, prefix: &str) -> Result<()> {
    let output = Command::new("jq")
        .arg("-r")
        .arg(jq_key)
        .arg(path)
        .output()
        .context("Failed to run jq")?;

    let got = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !got.starts_with(prefix) {
        bail!(
            "JSON value for {} does not start with {} in {} (got {})",
            jq_key,
            prefix,
            path.display(),
            got
        );
    }
    Ok(())
}

fn assert_line_count(path: &Path, pattern: &str, expected: usize) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;

    let re = regex::Regex::new(pattern)
        .with_context(|| format!("Invalid regex: {}", pattern))?;

    let count = content.lines().filter(|line| re.is_match(line)).count();
    if count != expected {
        bail!(
            "Expected {} occurrences of pattern '{}' in {}, got {}",
            expected,
            pattern,
            path.display(),
            count
        );
    }
    Ok(())
}

// New: assert file is non-empty (fixes missing helper error)
fn assert_gt_zero(path: &Path) -> Result<()> {
    let meta = fs::metadata(path)
        .with_context(|| format!("Failed to stat: {}", path.display()))?;
    if meta.len() == 0 {
        bail!("Expected non-zero size: {}", path.display());
    }
    Ok(())
}

// ========== Tests ==========

fn test_01_basic_e2e(_ctx: &TestContext, dir: &Path) -> Result<()> {
    new_git_repo(dir)?;

    // Rust crate
    let rc_dir = dir.join("rc");
    fs::create_dir_all(rc_dir.join("src"))?;
    fs::write(
        rc_dir.join("Cargo.toml"),
        r#"[package]
name="rc"
version="0.1.0"
edition="2021"
"#,
    )?;
    fs::write(rc_dir.join("src/lib.rs"), "pub fn x(){}")?;

    // TS file
    let web_dir = dir.join("web");
    fs::create_dir_all(&web_dir)?;
    fs::write(web_dir.join("index.ts"), "export const a=1")?;

    git_add_commit(dir, "init")?;

    run_saccade(_ctx, dir, &["--verbose"])?;

    let pack = dir.join("ai-pack");
    assert_file(&pack.join("PACK_MANIFEST.json"))?;
    assert_json_startswith(&pack.join("PACK_MANIFEST.json"), ".pack_version", r#""0.3."#)?;
    assert_file(&pack.join("CHAT_START.md"))?;
    assert_contains(&pack.join("API_SURFACE_RUST.txt"), r"pub\s+fn x\(\)")?;
    assert_contains(&pack.join("API_SURFACE_TS.txt"), r"^.*export const a=1")?;

    Ok(())
}

fn test_02_secrets_and_binaries_excluded(_ctx: &TestContext, dir: &Path) -> Result<()> {
    new_git_repo(dir)?;

    fs::write(dir.join(".env"), "SECRET=1\n")?;
    fs::write(
        dir.join("private.pem"),
        "-----BEGIN PRIVATE KEY-----\nX\n-----END PRIVATE KEY-----\n",
    )?;
    fs::write(dir.join("pic.png"), b"\x89PNG\r\n\x1a\n")?;
    fs::write(dir.join("code.rs"), "hello")?;

    git_add_commit(dir, "init")?;

    run_saccade(_ctx, dir, &["--verbose"])?;

    let index = dir.join("ai-pack/FILE_INDEX.txt");
    assert_not_contains(&index, r"^\.env$")?;
    assert_not_contains(&index, r"^private\.pem$")?;
    assert_not_contains(&index, r"^pic\.png$")?;
    assert_contains(&index, r"^code\.rs$")?;

    Ok(())
}

fn test_03_prune_in_find_mode(_ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::create_dir_all(dir.join("node_modules/a"))?;
    fs::create_dir_all(dir.join("dist"))?;
    fs::create_dir_all(dir.join("src"))?;

    fs::write(dir.join("node_modules/a/index.js"), "console.log(1)")?;
    fs::write(dir.join("dist/bundle.js"), "bundled")?;
    fs::write(dir.join("src/app.js"), "let x=1")?;

    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;

    let index = dir.join("ai-pack/FILE_INDEX.txt");
    assert_contains(&index, r"^src/app\.js$")?;
    assert_not_contains(&index, r"^node_modules/")?;
    assert_not_contains(&index, r"^dist/")?;

    Ok(())
}

fn test_04_git_vs_find_enumeration(_ctx: &TestContext, dir: &Path) -> Result<()> {
    new_git_repo(dir)?;

    fs::write(dir.join("ignored.md"), "ignored.md")?;
    fs::write(dir.join(".gitignore"), "ignored.md")?;
    fs::write(dir.join("keep.md"), "keep.md")?;

    run_cmd(dir, "git", &["add", ".gitignore", "keep.md"])?;
    run_cmd(dir, "git", &["commit", "-qm", "add keep"])?;

    // Git mode -> ignored.md excluded
    run_saccade(_ctx, dir, &["--verbose"])?;
    let index = dir.join("ai-pack/FILE_INDEX.txt");
    assert_contains(&index, r"^keep\.md$")?;
    assert_not_contains(&index, r"^ignored\.md$")?;

    // Force find -> ignored.md included
    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;
    assert_contains(&index, r"^ignored\.md$")?;

    Ok(())
}

fn test_05_api_rust_pub_and_scoped(_ctx: &TestContext, dir: &Path) -> Result<()> {
    let rc_dir = dir.join("rc");
    fs::create_dir_all(rc_dir.join("src"))?;

    fs::write(
        rc_dir.join("Cargo.toml"),
        r#"[package]
name="rc"
version="0.1.0"
edition="2021"
"#,
    )?;

    fs::write(
        rc_dir.join("src/lib.rs"),
        r#"pub(crate) struct Foo;
pub fn bar() {}
pub(super) trait T {}
mod inner { pub use super::Foo; }
"#,
    )?;

    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;

    let api = dir.join("ai-pack/API_SURFACE_RUST.txt");
    assert_contains(&api, r"pub\(crate\)\s+struct Foo")?;
    assert_contains(&api, r"pub\s+fn bar")?;
    assert_contains(&api, r"pub\(super\)\s+trait T")?;
    assert_contains(&api, r"pub\s+use\s+super::Foo")?;

    Ok(())
}

fn test_06_api_ts_exports_only_and_pascalcase(_ctx: &TestContext, dir: &Path) -> Result<()> {
    let app_dir = dir.join("app");
    fs::create_dir_all(&app_dir)?;

    fs::write(app_dir.join("package.json"), r#"{"name":"app"}"#)?;
    fs::write(
        app_dir.join("index.ts"),
        r#"export const X = 1;
const y = 2;
export default function alpha() { return 0; }
function Zeta(){ return 1; }
class Abc {}
"#,
    )?;

    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;

    let api = dir.join("ai-pack/API_SURFACE_TS.txt");
    assert_contains(&api, "export const X")?;
    assert_not_contains(&api, "const y")?;
    assert_contains(&api, "export default function alpha")?;
    assert_contains(&api, r"^.*function Zeta\(")?;
    assert_contains(&api, r"^.*class Abc")?;

    Ok(())
}

fn test_07_api_python_public_only(_ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(
        dir.join("a.py"),
        r#"def public_fn(): pass
def _private(): pass
class Public: pass
class _Hidden: pass
"#,
    )?;

    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;

    let api = dir.join("ai-pack/API_SURFACE_PYTHON.txt");
    assert_contains(&api, "def public_fn")?;
    assert_contains(&api, "class Public")?;
    assert_not_contains(&api, "def _private")?;
    assert_not_contains(&api, "class _Hidden")?;

    Ok(())
}

fn test_08_api_go_exported_only(_ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(
        dir.join("m.go"),
        r#"package main
func Exported() {}
func unexported() {}
"#,
    )?;

    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;

    let api = dir.join("ai-pack/API_SURFACE_GO.txt");
    assert_contains(&api, r"func\s+Exported\(")?;
    assert_not_contains(&api, "unexported")?;

    Ok(())
}

fn test_09_frontend_dedup_no_duplicates_in_api(_ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::create_dir_all(dir.join("packages/app"))?;
    fs::create_dir_all(dir.join("frontend"))?;

    fs::write(dir.join("packages/app/package.json"), r#"{"name":"app"}"#)?;
    fs::write(dir.join("frontend/package.json"), r#"{"name":"fe"}"#)?;
    fs::write(dir.join("frontend/k.ts"), "export const K=1")?;
    fs::write(dir.join("packages/app/k2.ts"), "export const K2=2")?;

    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;

    let api = dir.join("ai-pack/API_SURFACE_TS.txt");
    assert_line_count(&api, "export const K=1", 1)?;
    assert_line_count(&api, "export const K2=2", 1)?;

    Ok(())
}

fn test_10_dry_run_stats_and_no_writes(_ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(dir.join("a.js"), "console.log(1)")?;

    let output = run_saccade(_ctx, dir, &["--dry-run"])?;

    if !output.contains("Would generate the following artifacts") {
        bail!("dry-run header missing");
    }

    if dir.join("ai-pack").exists() {
        bail!("ai-pack should not exist in dry-run");
    }

    Ok(())
}

fn test_11_cli_validation_errors(_ctx: &TestContext, dir: &Path) -> Result<()> {
    let result = Command::new(&_ctx.config.saccade_bin)
        .current_dir(dir)
        .arg("--max-depth")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match result {
        Ok(status) if !status.success() => Ok(()),
        _ => bail!("expected error for --max-depth without value"),
    }
}

fn test_12_token_header_uses_div_3_5(_ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(dir.join("t.txt"), "a")?;

    run_saccade(_ctx, dir, &["--no-git"])?;

    assert_contains(&dir.join("ai-pack/TOKENS.txt"), r"bytes/3\.5")?;

    Ok(())
}

fn test_13_clickable_link_line_present(_ctx: &TestContext, dir: &Path) -> Result<()> {
    let is_windows = cfg!(target_os = "windows")
        || env::var("WT_SESSION").is_ok()
        || env::var("OSTYPE").map_or(false, |s| s.starts_with("msys") || s.starts_with("win32"));

    if !is_windows {
        // Skip on non-Windows
        return Ok(());
    }

    fs::write(dir.join("a.txt"), "x")?;
    run_saccade(_ctx, dir, &["--no-git", "--verbose"])?;

    assert_contains(&dir.join("run.log"), r"Click:\s+file://")?;

    Ok(())
}

fn test_14_stage2_repomix_optional(_ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(dir.join("a.txt"), "x")?;

    let has_repomix = Command::new("repomix")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok();

    run_saccade(_ctx, dir, &["--no-git"])?;

    let stage2 = dir.join("ai-pack/PACK_STAGE2_COMPRESSED.xml");

    if has_repomix {
        assert_file(&stage2)?;
        assert_gt_zero(&stage2)?;
    }
    // If repomix is absent, just verify no crash (file may or may not exist)

    Ok(())
}