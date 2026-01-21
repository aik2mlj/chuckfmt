# 🎵 chuckfmt

A fast code formatter for [ChucK](https://chuck.stanford.edu/) — the strongly-timed audio programming language.

## ✨ What it does

ChucK's syntax includes unique operators like `=>`, `@=>`, `<<<`/`>>>`, and `-->` that `clang-format` doesn't understand natively. **chuckfmt** wraps `clang-format` and applies ChucK-specific post-processing to fix these.

| Operator            | clang-format output | chuckfmt output |
| ------------------- | ------------------- | --------------- |
| ChucK operator      | `= >`               | `=>`            |
| UnChuck operator    | `= <`               | `=<`            |
| At-chuck            | `@ =>`              | `@=>`           |
| UpChucK operator    | `= ^ x`             | `=^ x`          |
| Time literal        | `1 ::second`        | `1::second`     |
| Debug print (open)  | `<<<x`              | `<<< x`         |
| Debug print (close) | `x>>>;`             | `x >>>;`        |
| .....               | `% (`               | `%(`            |
| gruck operator      | `-- >`              | `-->`           |

## 🚀 Installation

### From source (requires Rust)

```bash
cargo install --path .
```

### Pre-built binaries

Download from [Releases](../../releases) for:

- 🐧 Linux (x86_64, aarch64, musl)
- 🍎 macOS (Intel, Apple Silicon)
- 🪟 Windows (x86_64, aarch64)

### Requirements

`clang-format` must be available:

- **On PATH**: Just install `clang-format` via your package manager
- **Custom path**: Set `CLANG_FORMAT_BIN=/path/to/clang-format`

## 📖 Usage

```bash
# Format file to stdout
chuckfmt foo.ck

# Format multiple files to stdout
chuckfmt foo.ck bar.ck

# Format in-place
chuckfmt -i foo.ck bar.ck

# Pipe from stdin
cat foo.ck | chuckfmt

# Use a file list
chuckfmt -i --files filelist.txt

# Explicit file delimiter (useful for files starting with -)
chuckfmt -i --style=LLVM -- foo.ck bar.ck
```

All `clang-format` options are passed through. Run `clang-format --help` for details.

## ⚙️ Configuration

chuckfmt uses `clang-format`'s [configuration system](https://clang.llvm.org/docs/ClangFormatStyleOptions.html). This might be a good starting point (`.clang-format` file):

```yaml
BasedOnStyle: LLVM
ColumnLimit: 100
IndentWidth: 4
UseTab: Never
```

The formatter automatically adds `--assume-filename=code.cs` if not specified, which tells `clang-format` to use C#-like formatting rules (a reasonable approximation for ChucK).

## 🔧 How it works

1. Reads ChucK source code (from file or stdin)
2. Pipes it through `clang-format` with appropriate options
3. Applies regex-based transforms to fix ChucK-specific operators
4. Outputs the result (to stdout or overwrites the file with `-i`)

## 🧪 Testing

A test script is included to verify formatting doesn't break ChucK syntax:

```bash
python scripts/test_chuckfmt_syntax.py \
    --src /path/to/chuck/examples \
    --format-cmd "chuckfmt -i {}" \
    --chuck /path/to/chuck \
    --chuck-args=--silent \
    --timeout 0.5 \
    --jobs 12
```

This runs syntax checks on all `.ck` files before and after formatting, reporting any regressions.

I've run this against the [official ChucK examples](https://chuck.stanford.edu/doc/examples/) and it passes without issues.

## 📜 License

[MIT](LICENSE) © Lejun Min
