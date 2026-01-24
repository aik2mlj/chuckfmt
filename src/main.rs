use lazy_regex::regex_replace_all;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn apply_pre_formatting_transforms(s: &str) -> String {
    // add a ";" after @import statements to help clang-format parse them correctly
    let s = regex_replace_all!(r#"(?m)^(\s*@import\s*\{?\s*".*"\s*?\}?\s*?)$"#, &s, "$1;");
    s.into_owned()
}

/// Applies ChucK-specific formatting transforms to the input string.
fn apply_transforms(s: &str) -> String {
    // Allow any whitespace (including newlines) between tokens, because clang-format can emit "=\n  >".
    let s = regex_replace_all!(r"=\s*>", &s, "=>");
    let s = regex_replace_all!(r"=\s*<", &s, "=<");
    let s = regex_replace_all!(r"@\s*=>", &s, "@=>");
    let s = regex_replace_all!(r"=\s*\^\s*", &s, "=^ ");
    let s = regex_replace_all!(r"([0-9]+(?:\.[0-9]*)?)\s+::", &s, "$1::");
    let s = regex_replace_all!(r"<<<\s*", &s, "<<< ");
    let s = regex_replace_all!(r"< < <\s*", &s, "<<< ");
    let s = regex_replace_all!(r"\s*>>>\s*;", &s, " >>>;");
    let s = regex_replace_all!(r"%\s*\(", &s, "%(");
    let s = regex_replace_all!(r"\s*-\s*-\s*>\s*", &s, " --> ");
    let s = regex_replace_all!(r"(?m)^([+-])\s+([0-9]+(?:\.[0-9]*)?|\.[0-9]+)", &s, "$1$2");
    let s = regex_replace_all!(r"spork\s*~\s*", &s, "spork ~ ");

    // remove the ";" we added after @import statements
    let s = regex_replace_all!(r#"(?m)^(\s*@import.*);$"#, &s, "$1");
    s.into_owned()
}

// -------------------- Main --------------------

fn main() {
    if let Err(e) = real_main() {
        eprintln!("{}: {e}", env!("CARGO_PKG_NAME"));
        std::process::exit(1);
    }
}

/// Matches your bash wrapper behavior:
/// - Parse args into opts + files (supports `--` delimiter; heuristic otherwise)
/// - If user didn't provide assume-filename, append `--assume-filename=code.java`
/// - Without `-i`:
///   - If no files: read stdin, run clang-format on stdin, transforms, stdout
///   - If files: for each file, run clang-format on stdin (file contents), transforms, stdout
/// - With `-i`:
///   - Requires at least one file
///   - For each file: run clang-format on stdin (file contents) with opts (minus -i), transforms, overwrite file
fn real_main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let clang_format = resolve_clang_format()?;

    let has_inplace = args.iter().any(|a| a == "-i");

    let (mut opts, mut files) = split_opts_files(&args);
    expand_files_from_list(&opts, &mut files)?;

    if !has_assume_filename(&opts) {
        opts.push("--assume-filename=code.java".to_string());
    }

    if !has_inplace {
        // If no files detected, behave like clang-format: stdin -> stdout
        if files.is_empty() {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| format!("failed to read stdin: {e}"))?;

            let fixed = process_string(&clang_format, &opts, &input)?;

            io::stdout()
                .write_all(fixed.as_bytes())
                .map_err(|e| format!("failed to write stdout: {e}"))?;
            return Ok(());
        }

        // Files provided: format each file via stdin and write to stdout
        let mut out = io::stdout();
        for f in files {
            let input = fs::read_to_string(&f)
                .map_err(|e| format!("failed to read {}: {e}", f.display()))?;

            let fixed = process_string(&clang_format, &opts, &input)?;

            out.write_all(fixed.as_bytes())
                .map_err(|e| format!("failed to write stdout: {e}"))?;
        }
        return Ok(());
    }

    // In-place mode: require at least one file
    if files.is_empty() {
        return Err("chuckfmt: -i requires at least one file".to_string());
    }

    // Remove -i from options for the stdin formatting path
    let opts_no_i: Vec<String> = opts.into_iter().filter(|o| o != "-i").collect();

    for f in files {
        let input =
            fs::read_to_string(&f).map_err(|e| format!("failed to read {}: {e}", f.display()))?;

        let fixed = process_string(&clang_format, &opts_no_i, &input)?;

        // Match bash behavior: overwrite the file (no "only if changed" optimization)
        fs::write(&f, fixed).map_err(|e| format!("failed to write {}: {e}", f.display()))?;
    }

    Ok(())
}

// -------------------- Arg parsing (opts + files) --------------------

fn has_assume_filename(opts: &[String]) -> bool {
    opts.iter().any(|o| {
        o == "--assume-filename"
            || o == "-assume-filename"
            || o.starts_with("--assume-filename=")
            || o.starts_with("-assume-filename=")
    })
}

/// Mirrors your bash wrapper parsing:
/// - If `--` exists: everything before is opts, everything after is files (ignoring "-" and "--")
/// - Else heuristic:
///   - options that take a separate value set skip_next and both tokens go into opts
///   - tokens starting with '@', '-' (including "-") go into opts
///   - everything else goes into files
fn split_opts_files(args: &[String]) -> (Vec<String>, Vec<PathBuf>) {
    if let Some(pos) = args.iter().position(|a| a == "--") {
        let opts = args[..pos].to_vec();
        let mut files = Vec::new();
        for tok in &args[pos + 1..] {
            if tok == "-" || tok == "--" {
                continue;
            }
            files.push(PathBuf::from(tok));
        }
        return (opts, files);
    }

    let value_takers = [
        "-Wno-error",
        "--Wno-error",
        "-assume-filename",
        "--assume-filename",
        "-cursor",
        "--cursor",
        "-fallback-style",
        "--fallback-style",
        "-ferror-limit",
        "--ferror-limit",
        "-files",
        "--files",
        "-length",
        "--length",
        "-lines",
        "--lines",
        "-offset",
        "--offset",
        "-qualifier-alignment",
        "--qualifier-alignment",
        "-style",
        "--style",
    ];

    let mut opts = Vec::new();
    let mut files = Vec::new();
    let mut skip_next = false;

    for tok in args {
        if skip_next {
            skip_next = false;
            opts.push(tok.clone());
            continue;
        }

        if value_takers.contains(&tok.as_str()) {
            skip_next = true;
            opts.push(tok.clone());
            continue;
        }

        if tok.starts_with('@') || tok == "-" || tok.starts_with('-') {
            opts.push(tok.clone());
            continue;
        }

        files.push(PathBuf::from(tok));
    }

    (opts, files)
}

// -------------------- --files list expansion (no dedup) --------------------

fn expand_files_from_list(opts: &[String], files: &mut Vec<PathBuf>) -> Result<(), String> {
    // Expand --files <listfile> / --files=<listfile> and -files variants
    if let Some(listfile) = find_option_value_in(opts, "--files", "-files") {
        add_files_from_list(files, &listfile)?;
    }
    Ok(())
}

fn find_option_value_in(opts: &[String], long: &str, short: &str) -> Option<String> {
    // --opt=value / -opt=value
    for a in opts {
        if let Some(rest) = a.strip_prefix(&(long.to_string() + "=")) {
            return Some(rest.to_string());
        }
        if let Some(rest) = a.strip_prefix(&(short.to_string() + "=")) {
            return Some(rest.to_string());
        }
    }
    // --opt value / -opt value (within opts slice)
    let mut i = 0usize;
    while i < opts.len() {
        let a = &opts[i];
        if a == long || a == short {
            if i + 1 < opts.len() {
                return Some(opts[i + 1].clone());
            } else {
                return None;
            }
        }
        i += 1;
    }
    None
}

fn add_files_from_list(out: &mut Vec<PathBuf>, listfile: &str) -> Result<(), String> {
    let content = fs::read_to_string(listfile)
        .map_err(|e| format!("failed to read --files list '{}': {e}", listfile))?;
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        out.push(PathBuf::from(t));
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

// -------------------- Running clang-format (stdin -> stdout capture) --------------------

fn process_string(clang_format: &Path, opts: &[String], input: &str) -> Result<String, String> {
    let pre_formatted = apply_pre_formatting_transforms(input);
    let formatted = run_clang_format_on_stdin_capture(clang_format, opts, &pre_formatted)?;
    Ok(apply_transforms(&formatted))
}

/// Runs clang-format by sending `input` to stdin, capturing stdout as a String.
///
/// stderr is inherited (like your bash wrapper; useful for warnings).
fn run_clang_format_on_stdin_capture(
    clang: &Path,
    opts: &[String],
    input: &str,
) -> Result<String, String> {
    let mut child = Command::new(clang)
        .args(opts)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to launch clang-format: {e}"))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open clang-format stdin".to_string())?;
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| format!("failed writing clang-format stdin: {e}"))?;
    }

    let mut out = String::new();
    child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture clang-format stdout".to_string())?
        .read_to_string(&mut out)
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
