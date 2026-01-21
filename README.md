# 🎵 chuckfmt

A code formatter for [ChucK](https://chuck.stanford.edu/) — the audio programming language.

## ✨ What it does

Wraps `clang-format` and applies ChucK-specific fixes:

| Transform      | Before       | After       |
| -------------- | ------------ | ----------- |
| Stitch arrow   | `= >`        | `=>`        |
| At-arrow       | `@ =>`       | `@=>`       |
| Scope operator | `1 ::second` | `1::second` |
| Debug output   | `<<<x>>>`    | `<<< x >>>` |

## 🚀 Installation

```bash
cargo install --path .
```

Requires `clang-format` on your PATH (or set `CLANG_FORMAT_BIN`).

## 📖 Usage

```bash
# Format to stdout
chuckfmt foo.ck

# Format in-place
chuckfmt -i foo.ck bar.ck

# Pipe from stdin
cat foo.ck | chuckfmt
```

All `clang-format` options are passed through.

## ⚙️ Formatting style configuration

Use a `.clang-format` file in your project. Recommended starting point:

```yaml
BasedOnStyle: LLVM
ColumnLimit: 100
```
