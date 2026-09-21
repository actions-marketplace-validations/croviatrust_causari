# ∵ causari

**AI-written code has no author. It has causes. Causari proves them.**

`re audit` reads plain git history, finds the commits that carry
machine-readable AI authorship (`Co-Authored-By` trailers, bot authors, agent
markers) and asks `git blame` how many of their lines are still at HEAD. A
count, not a grade. The same binary records the prompt, model and files behind
every agent edit into a local, append-only ledger.

```sh
pipx run causari audit              # the repo you are in
pipx run causari audit vercel/next.js
pip install causari && re audit     # `re` is the short alias every example uses
```

This package is a launcher, not the program. It holds no binary and does
nothing at `pip install`. On first run it downloads the release archive for
your platform from
`https://github.com/croviatrust/causari/releases/download/v<version>/`,
checks its SHA-256 against the release's `SHA256SUMS.txt`, unpacks `causari`
and `re` into `~/.cache/causari/<version>/<target>/` (`$XDG_CACHE_HOME` or
`%LOCALAPPDATA%` when set) and replaces itself with it (`execv`; a child
process on Windows). Exit code passes through. Standard library only; no
telemetry.

Prebuilt binaries exist for Linux (x86_64, aarch64), macOS (x86_64, Apple
silicon) and Windows (x86_64). Elsewhere, `cargo install causari --locked`.

| Variable | Effect |
|---|---|
| `CAUSARI_VERSION` | run another release than the package version |
| `CAUSARI_BINARY` | path to a binary you already have; nothing is downloaded |
| `CAUSARI_DOWNLOAD_BASE` | mirror of the release directory (same layout, same `SHA256SUMS.txt`) |

Site and method: [causari.dev](https://causari.dev) ·
[causari.dev/method](https://causari.dev/method). Source, other installers and
the full command reference: [github.com/croviatrust/causari](https://github.com/croviatrust/causari).
Apache-2.0.
