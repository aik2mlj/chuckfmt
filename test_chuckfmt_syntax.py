#!/usr/bin/env python3
import argparse
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Dict, List, Tuple


def find_ck_files(root: Path) -> List[Path]:
    return sorted([p for p in root.rglob("*.ck") if p.is_file()])


def gather_with_progress(futures, total: int, label: str, every: int = 25):
    """
    Yield each future.result() while printing an in-place progress counter to stderr.
    """
    done = 0
    for fut in as_completed(futures):
        done += 1
        if done == 1 or done == total or (every > 0 and done % every == 0):
            print(f"\r{label}: {done}/{total}", end="", file=sys.stderr, flush=True)
        yield fut.result()
    print(file=sys.stderr)  # newline after finishing the phase


def to_text(x) -> str:
    if x is None:
        return ""
    if isinstance(x, bytes):
        return x.decode("utf-8", "replace")
    return x


def chuck_check_one(
    chuck_bin: str,
    chuck_args: List[str],
    file_path: Path,
    timeout: float,
) -> Tuple[Path, str, int, str]:
    cmd = [chuck_bin] + chuck_args + [str(file_path)]

    try:
        cp = subprocess.run(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout,
        )
        out = to_text(cp.stdout)
        err = to_text(cp.stderr)
        combined = (out + "\n" + err).strip()

        haystack = (out + "\n" + err).lower()
        if "syntax error" in haystack:
            return file_path, "fail", cp.returncode, combined

        return file_path, "ok", cp.returncode, combined

    except subprocess.TimeoutExpired as e:
        out = to_text(e.stdout)
        err = to_text(e.stderr)
        combined = (out + "\n" + err + "\n[TIMEOUT]").strip()
        return file_path, "timeout", 124, combined


def format_one(format_cmd_template: str, file_path: Path) -> Tuple[Path, int, str]:
    """
    format_cmd_template must contain '{}' where the file path should go, e.g.:
      "chuckfmt -i {}"
    This is run via the shell so you can use any CLI syntax.
    """
    if "{}" not in format_cmd_template:
        raise ValueError("format-cmd must include '{}' placeholder for the file path")

    cmd_str = format_cmd_template.format(shlex.quote(str(file_path)))

    cp = subprocess.run(
        cmd_str,
        shell=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    msg = (cp.stdout + "\n" + cp.stderr).strip()
    return file_path, cp.returncode, msg


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Test that chuckfmt formatting does not introduce ChucK syntax errors."
    )
    ap.add_argument("--src", required=True, help="Path to example codebase root")
    ap.add_argument(
        "--format-cmd",
        required=True,
        help="Formatter command template with '{}' placeholder, e.g. \"chuckfmt -i {}\"",
    )
    ap.add_argument("--chuck", default="chuck", help="Path to chuck binary (default: chuck)")
    ap.add_argument(
        "--chuck-args",
        nargs=argparse.REMAINDER,
        default=[],
        help="Args passed to chuck after --chuck-args (no quoting needed). "
        "Example: --chuck-args --silent",
    )
    ap.add_argument(
        "--timeout",
        type=float,
        default=0.35,
        help="Seconds to allow chuck per file (default: 0.35). Timeout is treated as PASS.",
    )
    ap.add_argument(
        "--jobs",
        type=int,
        default=max(4, (os.cpu_count() or 8)),
        help="Parallel workers (default: max(4, cpu_count))",
    )
    ap.add_argument(
        "--copy-ignore",
        default=".git,node_modules,target,build,dist",
        help="Comma-separated dir names to ignore when copying (default: .git,node_modules,target,build,dist)",
    )
    ap.add_argument(
        "--progress-every",
        type=int,
        default=25,
        help="Print progress every N completed files per phase (default: 25)",
    )

    args = ap.parse_args()

    src_root = Path(args.src).resolve()
    if not src_root.exists():
        print(f"ERROR: --src does not exist: {src_root}", file=sys.stderr)
        return 2

    chuck_bin = args.chuck
    chuck_args = args.chuck_args  # already a list (REMAINDER)

    ignore_names = {s.strip() for s in args.copy_ignore.split(",") if s.strip()}

    def ignore_func(_dirpath: str, names: List[str]) -> List[str]:
        return [n for n in names if n in ignore_names]

    with tempfile.TemporaryDirectory(prefix="chuckfmt_syntax_") as td:
        work_root = Path(td) / "work"
        shutil.copytree(src_root, work_root, ignore=ignore_func, dirs_exist_ok=True)

        ck_files = find_ck_files(work_root)
        if not ck_files:
            print("No .ck files found.", file=sys.stderr)
            return 2

        print(f"Copied to: {work_root}")
        print(f"Found {len(ck_files)} .ck files")
        print(f"Baseline check: {chuck_bin} {' '.join(chuck_args)} (timeout={args.timeout}s)")
        print(f"Formatting with: {args.format_cmd}")

        # 1) Baseline check
        baseline: Dict[Path, Tuple[str, int, str]] = {}
        with ThreadPoolExecutor(max_workers=args.jobs) as ex:
            futs = [
                ex.submit(chuck_check_one, chuck_bin, chuck_args, p, args.timeout) for p in ck_files
            ]
            for p, status, rc, msg in gather_with_progress(
                futs, len(futs), "Baseline check", every=args.progress_every
            ):
                baseline[p] = (status, rc, msg)

        # 2) Format all files
        print("Running formatter...")
        format_failures: List[Tuple[Path, int, str]] = []
        with ThreadPoolExecutor(max_workers=args.jobs) as ex:
            futs = [ex.submit(format_one, args.format_cmd, p) for p in ck_files]
            for p, rc, msg in gather_with_progress(
                futs, len(futs), "Formatting", every=args.progress_every
            ):
                if rc != 0:
                    format_failures.append((p, rc, msg))

        if format_failures:
            print("\nFormatter failures:")
            for p, rc, msg in sorted(format_failures, key=lambda x: str(x[0])):
                rel = p.relative_to(work_root)
                print(f"  - {rel} (rc={rc})")
                if msg:
                    print("    ---")
                    for line in msg.splitlines()[:30]:
                        print(f"    {line}")
                    if len(msg.splitlines()) > 30:
                        print("    ... (truncated)")
            return 1

        # 3) Post-format check
        print("Re-checking syntax after formatting...")
        after: Dict[Path, Tuple[str, int, str]] = {}
        with ThreadPoolExecutor(max_workers=args.jobs) as ex:
            futs = [
                ex.submit(chuck_check_one, chuck_bin, chuck_args, p, args.timeout) for p in ck_files
            ]
            for p, status, rc, msg in gather_with_progress(
                futs, len(futs), "After check", every=args.progress_every
            ):
                after[p] = (status, rc, msg)

        # 4) Regressions: baseline ok/timeout -> after fail
        regressions: List[Tuple[Path, Tuple[str, int, str], Tuple[str, int, str]]] = []
        baseline_fails: List[Path] = []

        for p in ck_files:
            b = baseline[p]
            a = after[p]

            if b[0] == "fail":
                baseline_fails.append(p)
            else:
                if a[0] == "fail":
                    regressions.append((p, b, a))

        def count_status(d: Dict[Path, Tuple[str, int, str]]) -> Dict[str, int]:
            c = {"ok": 0, "timeout": 0, "fail": 0}
            for st, _, _ in d.values():
                c[st] = c.get(st, 0) + 1
            return c

        bcnt = count_status(baseline)
        acnt = count_status(after)

        print("\nSummary")
        print(f"  Baseline: ok={bcnt['ok']} timeout={bcnt['timeout']} fail={bcnt['fail']}")
        print(f"  After:    ok={acnt['ok']} timeout={acnt['timeout']} fail={acnt['fail']}")
        if baseline_fails:
            print(
                f"  Note: {len(baseline_fails)} files failed even before formatting (not counted as regressions)."
            )

        if regressions:
            print("\nREGRESSIONS (formatting introduced 'syntax error' in stdout):")
            for p, b, a in sorted(regressions, key=lambda x: str(x[0])):
                rel = p.relative_to(work_root)
                print(f"\n  - {rel}")
                print(f"    baseline: {b[0]} (rc={b[1]})")
                print(f"    after:    {a[0]} (rc={a[1]})")
                if a[2]:
                    print("    --- chuck output (after) ---")
                    for line in a[2].splitlines()[:60]:
                        print(f"    {line}")
                    if len(a[2].splitlines()) > 60:
                        print("    ... (truncated)")
            return 1

        print("\nPASS: No syntax regressions introduced by formatting.")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
