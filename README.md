# 🎵 chuckfmt

A fast code formatter for [ChucK](https://chuck.stanford.edu/) — the strongly-timed audio programming language.

ChucK's syntax includes unique operators like `=>`, `@=>`, `<<<`/`>>>`, and `-->` that `clang-format` doesn't understand natively. **chuckfmt** wraps `clang-format` and applies ChucK-specific post-processing to fix these.

## 🚀 Installation

`clang-format` must be installed first: <a href="https://repology.org/project/chuckfmt/versions"> <img src="https://repology.org/badge/vertical-allrepos/chuckfmt.svg" alt="Packaging status" align="right"> </a>

```bash
# Debian/Ubuntu
sudo apt install clang-format

# Fedora
sudo dnf install clang-tools-extra

# Arch
sudo pacman -S clang

# macOS
brew install clang-format

# Windows (winget)
winget install LLVM.LLVM

# Windows (scoop)
scoop install llvm
```

Or set `CLANG_FORMAT_BIN=/path/to/clang-format` to use a custom path.

#### 🍺 Homebrew (macOS/Linux)

```bash
brew install aik2mlj/tap/chuckfmt
```

#### 📦 AUR (Arch Linux)

```bash
# use pre-built binary
paru -S chuckfmt-bin
# or if you prefer, compile from source
paru -S chuckfmt
```

#### 🍦 Scoop (Windows)

```powershell
scoop bucket add aik2mlj https://github.com/aik2mlj/scoop-bucket; scoop install aik2mlj/chuckfmt
```

#### 🛠️ Cargo (All platforms)

```bash
# use pre-built binary
# you need to have cargo-binstall installed first
cargo binstall chuckfmt
# or compile from source
cargo install chuckfmt
```

#### ⬇️ Download from Releases (All platforms)

- Download the corresponding binary archive from [Releases](https://github.com/aik2mlj/canvas-downloader/releases)
- Decompress the archive file
- Move it to `$PATH` for easier access

## ⚙️ Configuration

chuckfmt uses `clang-format`'s [configuration system](https://clang.llvm.org/docs/ClangFormatStyleOptions.html). This might be a good starting point:

```yaml
BasedOnStyle: LLVM
ColumnLimit: 100
IndentWidth: 4
UseTab: Never
```

Place this in a `.clang-format` file under your project root (for per-project settings) or home directory (for global settings).

## 💻 VS Code Integration

To add `chuckfmt` as the custom formatter for ChucK files in VS Code:

1. Install the [Custom Local Formatters](https://marketplace.visualstudio.com/items?itemName=jkillian.custom-local-formatters) extension

2. Open your user settings via Command Palette (`Ctrl+Shift+P` or `F1`) → `Preferences: Open User Settings (JSON)`, add the following and save:

```json
    "customLocalFormatters.formatters": [
        { "command": "chuckfmt", "languages": ["chuck"] }
    ],
    "files.associations": { "*.ck": "chuck" },
    "[chuck]": {
        "editor.formatOnSave": true
    }
```

3. Now test it by opening a `.ck` file and saving — it should be auto-formatted!

If you don't like format on save for ChucK files, set `editor.formatOnSave` to `false` in the `[chuck]` block. You can always manually trigger `Format Document` (`Ctrl+Shift+I`).

## 🐱 Neovim Integration

Using [conform.nvim](https://github.com/stevearc/conform.nvim):

```lua
require("conform").setup({
  formatters_by_ft = {
    chuck = { "chuckfmt" },
  },
  formatters = {
    chuckfmt = {
      command = "chuckfmt",
      stdin = true,
    },
  },
})
```

## 🪝 pre-commit Hook

Auto-format `.ck` files before each commit with [pre-commit](https://pre-commit.com/). Make sure `chuckfmt` is installed, then add to your `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: chuckfmt
        name: chuckfmt
        entry: chuckfmt -i
        language: system
        files: \.ck$
```

Then run `pre-commit install` to enable the hook.

## 📖 Command-line Usage

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

All `clang-format` options are passed through. Run `chuckfmt --help` for details.

## 🔧 How it works

1. Reads ChucK source code (from file or stdin)
2. Applies pre-processing (e.g., temporarily modifies `@import` for clang-format compatibility)
3. Pipes it through `clang-format` with appropriate options
4. Applies regex-based transforms to fix ChucK-specific operators (comments are preserved)
5. Outputs the result (to stdout or overwrites the file with `-i`)


| Operator            | clang-format output | chuckfmt output |
| ------------------- | ------------------- | --------------- |
| ChucK operator      | `= >`               | `=>`            |
| UnChuck operator    | `= <`               | `=<`            |
| At-chuck            | `@ =>`              | `@=>`           |
| UpChucK operator    | `= ^ x`             | `=^ x`          |
| Time literal        | `1 ::second`        | `1::second`     |
| Debug print (open)  | `<<<x`              | `<<< x`         |
| Debug print (close) | `x>>>;`             | `x >>>;`        |
| Polar literal       | `% (`               | `%(`            |
| Spork (function)    | `spork ~foo`        | `spork ~ foo`   |
| Gruck operator      | `-- >`              | `-->`           |
| Ungruck operator    | `-- <`              | `--<`           |
| Multiplication      | `2 *b`              | `2 * b`         |
| Leading sign        | `- 3.14`            | `-3.14`         |
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
