use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// Allow any whitespace (including newlines) between tokens, because clang-format can emit "=\n  >".
static RE_STITCH_ARROW: Lazy<Regex> = Lazy::new(|| Regex::new(r"=\s*>").unwrap());
static RE_STITCH_AT_ARROW: Lazy<Regex> = Lazy::new(|| Regex::new(r"@\s*=>").unwrap());
static RE_NUM_SPACE_COLON: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([0-9]+(?:\.[0-9]*)?)\s+::").unwrap());
static RE_PAD_LSHIFT: Lazy<Regex> = Lazy::new(|| Regex::new(r"<<<\s*").unwrap());
static RE_PAD_RSHIFT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*>>>").unwrap());

/// Applies ChucK-specific formatting transforms to the input string.
///
/// These transforms fix clang-format's output for ChucK syntax:
/// - `= >` → `=>` (stitch arrow)
/// - `@ =>` → `@=>` (at-arrow)
/// - `<digit> ::` → `<digit>::` (remove space before scope operator after digits)
/// - `<<<...` → `<<< ` (normalize debugging output operator padding)
/// - `...>>>` → ` >>>` (normalize debugging output operator padding)
fn apply_transforms(s: &str) -> String {
    let s = RE_STITCH_ARROW.replace_all(s, "=>");
    let s = RE_STITCH_AT_ARROW.replace_all(&s, "@=>");
    let s = RE_NUM_SPACE_COLON.replace_all(&s, "$1::");
    let s = RE_PAD_LSHIFT.replace_all(&s, "<<< ");
    let s = RE_PAD_RSHIFT.replace_all(&s, " >>>");
    s.into_owned()
}

// -------------------- Main --------------------

fn main() {
    if let Err(e) = real_main() {
        eprintln!("{}: {e}", env!("CARGO_PKG_NAME"));
        std::process::exit(1);
    }
}

/// Main entry point logic that handles both streaming and in-place modes.
///
/// Without `-i`: runs clang-format, applies transforms, writes to stdout.
/// With `-i`: runs clang-format in-place first, then post-processes each file.
fn real_main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let clang_format = resolve_clang_format()?;

    let has_inplace = args.iter().any(|a| a == "-i");

    if !has_inplace {
        // clang-format output -> transforms -> stdout
        let out = run_clang_format_capture(&clang_format, &args)?;
        // clang-format outputs text; treat as UTF-8 (common case).
        // If you need arbitrary encodings, we can switch to lossy decode.
        let text =
            String::from_utf8(out).map_err(|e| format!("clang-format output not UTF-8: {e}"))?;
        let fixed = apply_transforms(&text);
        io::stdout()
            .write_all(fixed.as_bytes())
            .map_err(|e| format!("failed to write stdout: {e}"))?;
        return Ok(());
    }

    // In-place: run clang-format first
    run_clang_format_passthrough(&clang_format, &args)?;

    // Post-process files edited in place
    let files = collect_files_for_inplace(&args)?;
    for f in files {
        postprocess_file(&f)?;
    }

    Ok(())
}

// -------------------- clang-format resolution --------------------

/// Locates the clang-format binary to use.
///
/// Resolution order:
/// 1. `CLANG_FORMAT_BIN` environment variable (must be executable)
/// 2. `clang-format` in PATH
fn resolve_clang_format() -> Result<PathBuf, String> {
    // 1) explicit override
    if let Ok(p) = env::var("CLANG_FORMAT_BIN") {
        let pb = PathBuf::from(p);
        if is_executable(&pb) {
            return Ok(pb);
        }
        return Err(format!(
            "CLANG_FORMAT_BIN is set but not executable: {}",
            pb.display()
        ));
    }

    // 2) PATH lookup
    if let Some(pb) = find_in_path(exe_name("clang-format")) {
        return Ok(pb);
    }

    Err(format!(
        "clang-format not found.\n\
         - Install clang-format and ensure it's on PATH, or\n\
         - Set CLANG_FORMAT_BIN to the full path of clang-format.\n\
         Example: CLANG_FORMAT_BIN=/usr/bin/clang-format {} ...",
        env!("CARGO_PKG_NAME")
    ))
}

fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn is_executable(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(p)
            .map(|m| (m.permissions().mode() & 0o111) != 0)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        true
    }
}

/// Searches for a program in the system PATH.
///
/// If `program` contains a path separator, treats it as a direct path.
/// On Windows, also tries appending `.exe` if not already present.
fn find_in_path(program: String) -> Option<PathBuf> {
    if program.contains(std::path::MAIN_SEPARATOR) {
        let p = PathBuf::from(program);
        return if is_executable(&p) { Some(p) } else { None };
    }

    let path_var: OsString = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(&program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        if cfg!(windows) && !program.to_lowercase().ends_with(".exe") {
            let candidate_exe = dir.join(format!("{program}.exe"));
            if is_executable(&candidate_exe) {
                return Some(candidate_exe);
            }
        }
    }
    None
}

// -------------------- Running clang-format --------------------

/// Runs clang-format and captures its stdout output.
///
/// Stdin and stderr are inherited from the parent process.
fn run_clang_format_capture(clang: &Path, args: &[String]) -> Result<Vec<u8>, String> {
    let mut child = Command::new(clang)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to launch clang-format: {e}"))?;

    let mut out = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture clang-format stdout".to_string())?
        .read_to_end(&mut out)
        .map_err(|e| format!("failed reading clang-format stdout: {e}"))?;

    let status = child
        .wait()
        .map_err(|e| format!("failed waiting for clang-format: {e}"))?;

    if !status.success() {
        return Err(format!(
            "clang-format failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(out)
}

/// Runs clang-format with all I/O inherited (passthrough mode).
///
/// Used for in-place formatting where clang-format writes directly to files.
fn run_clang_format_passthrough(clang: &Path, args: &[String]) -> Result<(), String> {
    let status = Command::new(clang)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("failed to run clang-format: {e}"))?;

    if !status.success() {
        return Err(format!(
            "clang-format failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

// -------------------- Post-processing files --------------------

fn postprocess_file(path: &Path) -> Result<(), String> {
    let original =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let updated = apply_transforms(&original);
    if updated != original {
        fs::write(path, updated).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }
    Ok(())
}

// -------------------- File collection for -i mode --------------------

/// Extracts the list of files that will be modified in-place.
///
/// Uses three strategies:
/// 1. Everything after `--` delimiter is treated as a file
/// 2. `--files <listfile>` reads filenames from a file
/// 3. Heuristic mode: non-option arguments are assumed to be files
///
/// If `--` is present, only strategies 1 and 2 are used (strict mode).
fn collect_files_for_inplace(args: &[String]) -> Result<Vec<PathBuf>, String> {
    // Use a BTreeSet to deduplicate and sort
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();

    // 1) `--` delimiter: everything after it is a file
    if let Some(pos) = args.iter().position(|a| a == "--") {
        for tok in &args[pos + 1..] {
            if tok == "-" || tok == "--" {
                continue;
            }
            files.insert(PathBuf::from(tok));
        }
    }

    // 2) --files <listfile> / --files=<listfile>
    if let Some(listfile) = find_option_value(args, "--files", "-files") {
        add_files_from_list(&mut files, &listfile)?;
    }

    // If delimiter exists, we’re done (strict mode)
    if args.iter().any(|a| a == "--") {
        return Ok(files.into_iter().collect());
    }

    // 3) Heuristic mode: non-option tokens are files, skipping option values
    let value_takers = [
        "--Wno-error",
        "-Wno-error",
        "--assume-filename",
        "-assume-filename",
        "--cursor",
        "-cursor",
        "--fallback-style",
        "-fallback-style",
        "--ferror-limit",
        "-ferror-limit",
        "--files",
        "-files",
        "--length",
        "-length",
        "--lines",
        "-lines",
        "--offset",
        "-offset",
        "--qualifier-alignment",
        "-qualifier-alignment",
        "--style",
        "-style",
    ];

    let mut skip_next = false;
    for tok in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if value_takers.contains(&tok.as_str()) {
            skip_next = true;
            continue;
        }
        if tok.starts_with('@') || tok == "-" || tok.starts_with('-') {
            continue;
        }
        files.insert(PathBuf::from(tok));
    }

    Ok(files.into_iter().collect())
}

/// Finds the value of a command-line option, supporting both `--opt=value` and `--opt value` forms.
fn find_option_value(args: &[String], long: &str, short: &str) -> Option<String> {
    // --opt=value
    for a in args {
        if let Some(rest) = a.strip_prefix(&(long.to_string() + "=")) {
            return Some(rest.to_string());
        }
        if let Some(rest) = a.strip_prefix(&(short.to_string() + "=")) {
            return Some(rest.to_string());
        }
    }
    // --opt value
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        if a == long || a == short {
            return it.peek().cloned().cloned();
        }
    }
    None
}

fn add_files_from_list(out: &mut BTreeSet<PathBuf>, listfile: &str) -> Result<(), String> {
    let content = fs::read_to_string(listfile)
        .map_err(|e| format!("failed to read --files list '{}': {e}", listfile))?;
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        out.insert(PathBuf::from(t));
    }
    Ok(())
}
