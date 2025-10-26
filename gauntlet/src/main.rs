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
        eprintln!("❌ Fatal error: {:#}", e);
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
        .unwrap_or_else(|_| "target/release/saccade".to_string())
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
    
    if !config.saccade_bin.exists() {
        bail!("SACCADE binary not found at: {}\nDid you run `cargo build --release` first?", config.saccade_bin.display());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata =
            fs::metadata(&config.saccade_bin).context("Failed to read saccade binary metadata")?;
        if metadata.permissions().mode() & 0o111 == 0 {
            bail!(
                "SACCADE not executable at: {}",
                config.saccade_bin.display()
            );
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
        (
            "test_02_secrets_and_binaries_excluded",
            test_02_secrets_and_binaries_excluded,
        ),
        ("test_03_prune_in_find_mode", test_03_prune_in_find_mode),
        (
            "test_04_git_vs_find_enumeration",
            test_04_git_vs_find_enumeration,
        ),
        (
            "test_05_api_rust_pub_and_scoped",
            test_05_api_rust_pub_and_scoped,
        ),
        (
            "test_06_api_ts_exports_only_and_pascalcase",
            test_06_api_ts_exports_only_and_pascalcase,
        ),
        (
            "test_07_api_python_public_only",
            test_07_api_python_public_only,
        ),
        ("test_08_api_go_exported_only", test_08_api_go_exported_only),
        (
            "test_09_frontend_dedup_no_duplicates_in_api",
            test_09_frontend_dedup_no_duplicates_in_api,
        ),
        (
            "test_10_dry_run_stats_and_no_writes",
            test_10_dry_run_stats_and_no_writes,
        ),
        ("test_11_cli_validation_errors", test_11_cli_validation_errors),
        (
            "test_12_token_header_uses_div_3_5",
            test_12_token_header_uses_div_3_5,
        ),
        (
            "test_13_clickable_link_line_present",
            test_13_clickable_link_line_present,
        ),
        ("test_14_stage2_optional", test_14_stage2_optional),
        ("test_15_structure_annotation", test_15_structure_annotation),
        ("test_16_multi_deps_synthesis", test_16_multi_deps_synthesis),
    ]
}

fn execute_tests(ctx: &mut TestContext, tests: &[(&str, TestFn)]) -> Result<()> {
    for (name, test_fn) in tests {
        if let Some(ref filter) = ctx.config.filter {
            if !name.contains(filter.as_str()) {
                if ctx.config.verbose {
                    println!("    skip {} due to filter", name);
                }
                ctx.stats.skip += 1;
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
                eprintln!("❌ {} failed: {:#}", name, e);
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
    let saccade_abs = fs::canonicalize(&ctx.config.saccade_bin)?;
    
    let output = Command::new(&saccade_abs)
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
    
    if !output.status.success() {
        bail!("Saccade exited with non-zero status:\n{}", combined);
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

    // Enable multiline mode for patterns using ^ or $
    let pattern_with_flags = if pattern.contains('^') || pattern.contains('$') {
        format!("(?m){}", pattern)
    } else {
        pattern.to_string()
    };

    let re = regex::Regex::new(&pattern_with_flags)
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

    // Enable multiline mode for patterns using ^ or $
    let pattern_with_flags = if pattern.contains('^') || pattern.contains('$') {
        format!("(?m){}", pattern)
    } else {
        pattern.to_string()
    };

    let re = regex::Regex::new(&pattern_with_flags)
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

fn assert_line_count(path: &Path, pattern: &str, expected: usize) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;

    let re = regex::Regex::new(pattern).with_context(|| format!("Invalid regex: {}", pattern))?;

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

fn assert_gt_zero(path: &Path) -> Result<()> {
    let meta =
        fs::metadata(path).with_context(|| format!("Failed to stat: {}", path.display()))?;
    if meta.len() == 0 {
        bail!("Expected non-zero size: {}", path.display());
    }
    Ok(())
}

// ========== Tests ==========

fn test_01_basic_e2e(ctx: &TestContext, dir: &Path) -> Result<()> {
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

    run_saccade(ctx, dir, &["--verbose"])?;

    let pack = dir.join("ai-pack/PACK.txt");
    
    // Check for consolidated sections in single file
    assert_contains(&pack, "=======PROJECT=======")?;
    assert_contains(&pack, "=======STRUCTURE=======")?;
    assert_contains(&pack, "=======APIS=======")?;
    
    // Check consolidated API surfaces
    assert_contains(&pack, r"pub\s+fn x\(\)")?;
    assert_contains(&pack, r"export const a=1")?;
    assert_contains(&pack, "API SURFACE: RUST")?;
    assert_contains(&pack, "API SURFACE: TYPESCRIPT/JAVASCRIPT")?;

    Ok(())
}

fn test_02_secrets_and_binaries_excluded(ctx: &TestContext, dir: &Path) -> Result<()> {
    new_git_repo(dir)?;

    fs::write(dir.join(".env"), "SECRET=1\n")?;
    fs::write(
        dir.join("private.pem"),
        "-----BEGIN PRIVATE KEY-----\nX\n-----END PRIVATE KEY-----\n",
    )?;
    fs::write(dir.join("pic.png"), b"\x89PNG\r\n\x1a\n")?;
    fs::write(dir.join("code.rs"), "hello")?;

    git_add_commit(dir, "init")?;

    run_saccade(ctx, dir, &["--verbose"])?;

    let structure = dir.join("ai-pack/PACK.txt");
    assert_not_contains(&structure, r"^\.env$")?;
    assert_not_contains(&structure, r"^private\.pem$")?;
    assert_not_contains(&structure, r"^pic\.png$")?;
    assert_contains(&structure, r"^code\.rs$")?;

    Ok(())
}

fn test_03_prune_in_find_mode(ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::create_dir_all(dir.join("node_modules/a"))?;
    fs::create_dir_all(dir.join("dist"))?;
    fs::create_dir_all(dir.join("src"))?;

    fs::write(dir.join("node_modules/a/index.js"), "console.log(1)")?;
    fs::write(dir.join("dist/bundle.js"), "bundled")?;
    fs::write(dir.join("src/app.js"), "let x=1")?;

    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    let structure = dir.join("ai-pack/PACK.txt");
    assert_contains(&structure, r"src/app\.js$")?;
    assert_not_contains(&structure, r"node_modules/")?;
    assert_not_contains(&structure, r"dist/")?;

    Ok(())
}

fn test_04_git_vs_find_enumeration(ctx: &TestContext, dir: &Path) -> Result<()> {
    new_git_repo(dir)?;

    fs::write(dir.join("ignored.md"), "ignored.md")?;
    fs::write(dir.join(".gitignore"), "ignored.md")?;
    fs::write(dir.join("keep.md"), "keep.md")?;

    run_cmd(dir, "git", &["add", ".gitignore", "keep.md"])?;
    run_cmd(dir, "git", &["commit", "-qm", "add keep"])?;

    // Git mode -> ignored.md excluded
    run_saccade(ctx, dir, &["--verbose"])?;
    let pack = dir.join("ai-pack/PACK.txt");
    assert_contains(&pack, r"keep\.md$")?;
    assert_not_contains(&pack, r"ignored\.md$")?;

    // Force find -> ignored.md included
    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;
    assert_contains(&pack, r"ignored\.md$")?;

    Ok(())
}

fn test_05_api_rust_pub_and_scoped(ctx: &TestContext, dir: &Path) -> Result<()> {
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

    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    let apis = dir.join("ai-pack/PACK.txt");
    assert_contains(&apis, r"pub\(crate\)\s+struct Foo")?;
    assert_contains(&apis, r"pub\s+fn bar")?;
    assert_contains(&apis, r"pub\(super\)\s+trait T")?;
    assert_contains(&apis, r"pub\s+use\s+super::Foo")?;

    Ok(())
}

fn test_06_api_ts_exports_only_and_pascalcase(ctx: &TestContext, dir: &Path) -> Result<()> {
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

    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    let apis = dir.join("ai-pack/PACK.txt");
    assert_contains(&apis, "export const X")?;
    assert_not_contains(&apis, "const y")?;
    assert_contains(&apis, "export default function alpha")?;
    assert_contains(&apis, r"function Zeta\(")?;
    assert_contains(&apis, r"class Abc")?;

    Ok(())
}

fn test_07_api_python_public_only(ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(
        dir.join("a.py"),
        r#"def public_fn(): pass
def _private(): pass
class Public: pass
class _Hidden: pass
"#,
    )?;

    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    let apis = dir.join("ai-pack/PACK.txt");
    assert_contains(&apis, "def public_fn")?;
    assert_contains(&apis, "class Public")?;
    assert_not_contains(&apis, "def _private")?;
    assert_not_contains(&apis, "class _Hidden")?;

    Ok(())
}

fn test_08_api_go_exported_only(ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(
        dir.join("m.go"),
        r#"package main
func Exported() {}
func unexported() {}
"#,
    )?;

    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    let apis = dir.join("ai-pack/PACK.txt");
    assert_contains(&apis, r"func\s+Exported\(")?;
    assert_not_contains(&apis, "unexported")?;

    Ok(())
}

fn test_09_frontend_dedup_no_duplicates_in_api(ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::create_dir_all(dir.join("packages/app"))?;
    fs::create_dir_all(dir.join("frontend"))?;

    fs::write(
        dir.join("packages/app/package.json"),
        r#"{"name":"app"}"#,
    )?;
    fs::write(dir.join("frontend/package.json"), r#"{"name":"fe"}"#)?;
    fs::write(dir.join("frontend/k.ts"), "export const K=1")?;
    fs::write(dir.join("packages/app/k2.ts"), "export const K2=2")?;

    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    let apis = dir.join("ai-pack/PACK.txt");
    assert_line_count(&apis, "export const K=1", 1)?;
    assert_line_count(&apis, "export const K2=2", 1)?;

    Ok(())
}

fn test_10_dry_run_stats_and_no_writes(ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(dir.join("a.js"), "console.log(1)")?;

    let output = run_saccade(ctx, dir, &["--dry-run"])?;

    if !output.contains("Would generate the following artifacts") {
        bail!("dry-run header missing");
    }

    if dir.join("ai-pack").exists() {
        bail!("ai-pack should not exist in dry-run");
    }

    Ok(())
}

fn test_11_cli_validation_errors(ctx: &TestContext, dir: &Path) -> Result<()> {
    let saccade_abs = fs::canonicalize(&ctx.config.saccade_bin)?;
    let result = Command::new(&saccade_abs)
        .current_dir(dir)
        .arg("--max-depth=99")
        .output();

    match result {
        Ok(output) if !output.status.success() => Ok(()),
        _ => bail!("expected error for --max-depth=99"),
    }
}

fn test_12_token_header_uses_div_3_5(ctx: &TestContext, dir: &Path) -> Result<()> {
    fs::write(dir.join("t.txt"), "a")?;

    run_saccade(ctx, dir, &["--no-git"])?;

    assert_contains(&dir.join("ai-pack/PACK.txt"), r"bytes → ~tokens via /3.5")?;

    Ok(())
}

fn test_13_clickable_link_line_present(ctx: &TestContext, dir: &Path) -> Result<()> {
    if !cfg!(target_os = "windows") {
        println!("    skip on non-Windows");
        return Ok(());
    }

    fs::write(dir.join("a.txt"), "x")?;
    let log = run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    assert!(log.contains("Click: file://"), "Clickable link not found in log");

    Ok(())
}

fn test_14_stage2_optional(ctx: &TestContext, dir: &Path) -> Result<()> {
    // Create a file that is parsable
    fs::write(dir.join("a.rs"), "pub fn test() {}")?;

    run_saccade(ctx, dir, &["--no-git"])?;

    let stage2 = dir.join("ai-pack/PACK_STAGE2_COMPRESSED.xml");

    assert_file(&stage2)?;
    assert_gt_zero(&stage2)?;
    
    // Create a file that is not parsable
    fs::write(dir.join("b.txt"), "not parsable content")?;
    
    run_saccade(ctx, dir, &["--no-git"])?;

    // Stage 2 should still exist from the .rs file, and the run should not crash.
    assert_file(&stage2)?;

    Ok(())
}

fn test_15_structure_annotation(ctx: &TestContext, dir: &Path) -> Result<()> {
    let cpp_dir = dir.join("cpp/app");
    fs::create_dir_all(&cpp_dir)?;
    fs::write(
        cpp_dir.join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.10)\nproject(MyApp)\nadd_executable(app main.cpp)",
    )?;
    fs::write(cpp_dir.join("main.cpp"), "int main() { return 0; }")?;

    run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    let pack = dir.join("ai-pack/PACK.txt");
    assert_file(&pack)?;

    // Assert that the directory tree contains the annotation for the parent dir.
    assert_contains(&pack, r"^cpp/app\s+<-- \[CMake Project\]$")?;

    Ok(())
}

fn test_16_multi_deps_synthesis(ctx: &TestContext, dir: &Path) -> Result<()> {
    // 1. Create CMake project with a dependency
    let cmake_dir = dir.join("math_lib");
    fs::create_dir_all(&cmake_dir)?;
    fs::write(
        cmake_dir.join("CMakeLists.txt"),
        "find_package(Boost 1.70.0 REQUIRED)",
    )?;

    // 2. Create a Conan project with a dependency and other Python code
    let conan_dir = dir.join("network_app");
    fs::create_dir_all(&conan_dir)?;
    let conanfile_content = r#"
from conan import ConanFile

# A comment that should be ignored by the parser
class MyNetworkApp(ConanFile):
    name = "network-app"
    version = "1.0"
    
    # The critical dependency line
    requires = "zlib/1.2.13"
"#;
    fs::write(
        conan_dir.join("conanfile.py"),
        conanfile_content,
    )?;

    // 3. Run Saccade
    let log = run_saccade(ctx, dir, &["--no-git", "--verbose"])?;

    // 4. Verify that the detector found both systems
    assert!(
        log.contains("Detected build systems: [CMake, Conan]") || log.contains("Detected build systems: [Conan, CMake]"),
        "Detector did not find both CMake and Conan"
    );

    let pack = dir.join("ai-pack/PACK.txt");
    assert_file(&pack)?;

    // 5. Verify that the DEPS section contains dependencies from BOTH sources
    assert_contains(&pack, "=======DEPS=======")?;
    // Check for CMake dependency
    assert_contains(&pack, r"- Boost")?;
    // Check for Conan dependency, parsed by Tree-sitter
    assert_contains(&pack, r"- zlib/1.2.13")?;
    // Check for correct headers
    assert_contains(&pack, r"C\+\+ \(CMake\)")?;
    assert_contains(&pack, r"C\+\+ \(Conan\)")?;

    Ok(())
}