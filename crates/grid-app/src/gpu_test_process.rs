//! Runs a test that opens a real GPU device in a child process of its test
//! binary (F922).
//!
//! The NVIDIA D3D12 user-mode driver (`nvwgf2umx.dll` 32.0.16.2101) faulted
//! with an access violation at +0x8040b2 in the grid-app test process minutes
//! after the one unit test that opens real devices had finished, while
//! unrelated CPU tests ran in parallel; the fault killed the whole libtest
//! process. The game opens one device per process and keeps it
//! (`front_end_host`), and so do the GPU integration tests (`tests/`, one
//! process each). Here every test body that creates a device runs in a fresh
//! run of the same test binary filtered to that one test, so the driver's
//! device state and threads live and die with a process that does nothing
//! else. The parent waits, echoes the child's output (libtest captures it)
//! and fails when the child does not report exactly one passed test.
//! [`tests::device_tests_run_in_their_own_process`] keeps every unit test
//! that opens a device in the workspace on this path.

use std::process::Command;

/// Set in the child to the test's path; the child then runs the body.
const CHILD: &str = "GRID_GPU_TEST_CHILD";

/// Runs `body` in a child run of this test binary limited to the calling test
/// (`--exact`), or directly when this process is that child. Call it first in
/// a `#[test]` function, on the test's own thread: libtest names the thread
/// after the test, and that name is the child's filter.
pub fn in_own_process(body: impl FnOnce()) {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or_default();
    if std::env::var_os(CHILD).is_some() {
        body();
        return;
    }
    assert!(
        !name.is_empty() && name != "main",
        "in_own_process needs a libtest test thread, got {name:?}; run with --test-threads above 1"
    );
    let exe = std::env::current_exe().expect("the test binary path");
    let output = Command::new(&exe)
        .args([name, "--exact", "--include-ignored", "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .output()
        .expect("start the test binary as a child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{stdout}");
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success(), "{name}: the child run failed ({})", output.status);
    assert!(stdout.contains("test result: ok. 1 passed"), "{name}: the child run did not run exactly this test");
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// Calls and plugins that open a real GPU device.
    const DEVICE: [&str; 6] =
        ["request_device(", "enumerate_adapters(", "request_adapter(", "RenderPlugin", "DefaultPlugins", "RenderCreation::"];

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// The `#[cfg(test)]` modules of `file`: an inline one from its start to
    /// the end of the file, an out-of-line one as its whole file.
    fn test_modules(file: &Path, text: &str) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        for (at, attribute) in text.match_indices("#[cfg(test)]") {
            let rest = &text[at + attribute.len()..];
            let mut item = rest.trim_start();
            while item.starts_with("#[") {
                let Some(end) = item.find(']') else { break };
                item = item[end + 1..].trim_start();
            }
            let Some(item) = item.strip_prefix("mod ") else { continue };
            let name: String = item.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let after = item[name.len()..].trim_start();
            if after.starts_with('{') {
                out.push((file.to_owned(), rest.to_owned()));
            } else if after.starts_with(';') {
                let parent = file.parent().unwrap_or(Path::new("."));
                let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let base = if matches!(stem, "main" | "lib" | "mod") { parent.to_owned() } else { parent.join(stem) };
                for candidate in [base.join(format!("{name}.rs")), base.join(&name).join("mod.rs")] {
                    if let Ok(module) = std::fs::read_to_string(&candidate) {
                        out.push((candidate, module));
                        break;
                    }
                }
            }
        }
        out
    }

    /// Every unit test module in the workspace's crates that opens a GPU
    /// device runs its device work through `in_own_process` (integration tests
    /// in `tests/` are processes of their own).
    #[test]
    fn device_tests_run_in_their_own_process() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates folder");
        let mut files = Vec::new();
        for entry in std::fs::read_dir(crates).expect("list the crates").flatten() {
            rust_files(&entry.path().join("src"), &mut files);
        }
        let (mut modules, mut with_device, mut outside) = (0, 0, Vec::new());
        for file in &files {
            let text = std::fs::read_to_string(file).expect("read a source file");
            for (path, module) in test_modules(file, &text) {
                modules += 1;
                if DEVICE.iter().any(|call| module.contains(call)) {
                    with_device += 1;
                    if !module.contains("in_own_process(") {
                        outside.push(path);
                    }
                }
            }
        }
        println!("test modules: {modules}, opening a device: {with_device}");
        assert!(modules > 300, "found only {modules} test modules: the scan is broken");
        assert!(with_device >= 1, "the scan no longer finds pane_slab's device test");
        assert!(outside.is_empty(), "these test modules open a GPU device in the shared test process: {outside:?}");
    }
}
