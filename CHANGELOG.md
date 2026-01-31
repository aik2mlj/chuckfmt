# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-01-31

### Added

- Preserve comments during transforms — `//` and `/* */` comments are no longer affected by regex replacements
- Polar literal formatting: `% (` → `%(`
- Ungruck operator formatting: `-- <` → `--<`
- Leading sign formatting: `- 1` → `-1` at start of lines
- Multiplication spacing: `2 *b` → `2 * b`
- Spork function formatting: `spork ~foo` → `spork ~ foo`
- Pre-processing for `@import` statements to improve clang-format compatibility
- VS Code integration guide (Run on Save extension)
- Neovim integration guide (conform.nvim)

### Changed

- Default `--assume-filename` changed from `code.cs` (C#) to `code.java` (Java) for better ChucK syntax approximation
- Switched from `regex` + `once_cell` crates to `lazy-regex` for cleaner code

## [0.1.0] - 2026-01-20

### Added

- Initial release
- Wrap `clang-format` with ChucK-specific post-processing
- Core operator transforms: `=>`, `=<`, `@=>`, `=^`, `::`, `<<<`/`>>>`, `-->`
- In-place editing with `-i` flag
- Stdin/stdout streaming mode
- `--files` option for file lists
- Cross-platform support (Linux, macOS, Windows)
- GitHub Actions release workflow

[0.2.0]: https://github.com/aik2mlj/chuckfmt/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/aik2mlj/chuckfmt/releases/tag/v0.1.0
