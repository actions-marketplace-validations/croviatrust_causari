"""Thin launcher for the causari release binaries.

On first run it downloads the archive for this platform from the GitHub
release that matches the installed package version, checks its SHA-256
against the release's SHA256SUMS.txt, unpacks ``causari`` and ``re`` into a
per-version cache and replaces itself with the requested one. Nothing
happens at ``pip install``: no network, no build step. Standard library only.

Environment:
  CAUSARI_VERSION        release to run (default: this package's version)
  CAUSARI_BINARY         path to an existing binary; skips the download
  CAUSARI_DOWNLOAD_BASE  where releases live (default: GitHub releases)
  XDG_CACHE_HOME / LOCALAPPDATA  cache root (default: ~/.cache)
"""

from __future__ import annotations

import hashlib
import io
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path
from typing import Callable, Dict, Mapping, Optional

REPO = "croviatrust/causari"
DEFAULT_BASE = f"https://github.com/{REPO}/releases/download"
TIMEOUT_S = 60

TARGETS = {
    ("linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("darwin", "x86_64"): "x86_64-apple-darwin",
    ("darwin", "aarch64"): "aarch64-apple-darwin",
    ("windows", "x86_64"): "x86_64-pc-windows-msvc",
}


class ShimError(Exception):
    """A failure the user can act on; printed as one line, exit 1."""


def log(msg: str) -> None:
    sys.stderr.write(f"causari: {msg}\n")
    sys.stderr.flush()


# ---------------------------------------------------------------- platform


def package_version() -> str:
    try:
        from importlib.metadata import PackageNotFoundError, version

        return version("causari")
    except PackageNotFoundError:
        pass
    except Exception:  # noqa: BLE001 - metadata backends vary; fall through
        pass
    # Source checkout without an installed distribution (tests, development).
    pyproject = Path(__file__).resolve().parent.parent / "pyproject.toml"
    m = re.search(r'^version = "([^"]+)"', pyproject.read_text(encoding="utf-8"), re.M)
    if not m:
        raise ShimError("cannot determine the causari version: package not installed and no pyproject.toml")
    return m.group(1)


def resolve_version(env: Mapping[str, str]) -> str:
    v = (env.get("CAUSARI_VERSION") or package_version()).strip()
    return v[1:] if v.startswith("v") else v


def target(system: Optional[str] = None, machine: Optional[str] = None) -> str:
    system = (system or platform.system()).lower()
    machine = (machine or platform.machine()).lower()
    machine = {"amd64": "x86_64", "x64": "x86_64", "arm64": "aarch64"}.get(machine, machine)
    t = TARGETS.get((system, machine))
    if not t:
        raise ShimError(
            f"no prebuilt binary for {system}/{machine}. Releases cover Linux (x86_64, aarch64), "
            "macOS (x86_64, Apple silicon) and Windows (x86_64). Build from source instead: "
            "cargo install causari --locked"
        )
    return t


def asset_name(version: str, tgt: str) -> str:
    ext = "zip" if tgt.endswith("-windows-msvc") else "tar.gz"
    return f"causari-v{version}-{tgt}.{ext}"


def exe_name(name: str, tgt: str) -> str:
    return f"{name}.exe" if tgt.endswith("-windows-msvc") else name


def cache_root(env: Mapping[str, str], system: Optional[str] = None) -> Path:
    if env.get("XDG_CACHE_HOME"):
        return Path(env["XDG_CACHE_HOME"])
    system = (system or platform.system()).lower()
    if system == "windows" and env.get("LOCALAPPDATA"):
        return Path(env["LOCALAPPDATA"])
    return Path.home() / ".cache"


def cache_dir(version: str, tgt: str, env: Mapping[str, str], system: Optional[str] = None) -> Path:
    return cache_root(env, system) / "causari" / version / tgt


# ---------------------------------------------------------------- download


def fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": f"causari-pypi-shim/{package_version()}", "Accept": "*/*"})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT_S) as resp:  # follows redirects
            return resp.read()
    except urllib.error.HTTPError as e:
        raise ShimError(f"HTTP {e.code} fetching {url}") from e
    except (urllib.error.URLError, OSError) as e:
        raise ShimError(f"{e} fetching {url}") from e


def expected_sum(sums_text: str, asset: str) -> str:
    """One line per file, ``<hex>  <name>`` or ``<hex> *<name>`` (sha256sum format)."""
    for raw in sums_text.splitlines():
        m = re.match(r"^([0-9a-fA-F]{64})\s+\*?(.+)$", raw.strip())
        if m and m.group(2).strip() == asset:
            return m.group(1).lower()
    raise ShimError(f"no checksum for {asset} in SHA256SUMS.txt; refusing to run an unverified download")


def extract(asset: str, data: bytes, wanted: list) -> Dict[str, bytes]:
    """Return the wanted files (matched on base name) from a .tar.gz or .zip."""
    out: Dict[str, bytes] = {}
    if asset.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(data)) as zf:
            for info in zf.infolist():
                base = Path(info.filename).name
                if base in wanted and not info.is_dir() and base not in out:
                    out[base] = zf.read(info)
    else:
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tf:
            for member in tf:
                base = Path(member.name).name
                if base in wanted and member.isfile() and base not in out:
                    f = tf.extractfile(member)
                    if f is not None:
                        out[base] = f.read()
    missing = [w for w in wanted if w not in out]
    if missing:
        raise ShimError(f"archive {asset} does not contain {', '.join(missing)}")
    return out


# ---------------------------------------------------------------- install


def ensure_binary(
    name: str,
    env: Optional[Mapping[str, str]] = None,
    fetcher: Callable[[str], bytes] = fetch,
    system: Optional[str] = None,
    machine: Optional[str] = None,
) -> Path:
    env = os.environ if env is None else env
    if env.get("CAUSARI_BINARY"):
        p = Path(env["CAUSARI_BINARY"])
        if not p.is_file():
            raise ShimError(f"CAUSARI_BINARY={p} does not exist")
        return p

    version = resolve_version(env)
    tgt = target(system, machine)
    directory = cache_dir(version, tgt, env, system)
    wanted = [exe_name(n, tgt) for n in ("causari", "re")]
    binary = directory / exe_name(name, tgt)
    if binary.is_file():
        return binary

    base = (env.get("CAUSARI_DOWNLOAD_BASE") or DEFAULT_BASE).rstrip("/")
    asset = asset_name(version, tgt)
    log(f"first run: downloading causari v{version} ({tgt}) from {base}/v{version}/")
    sums = fetcher(f"{base}/v{version}/SHA256SUMS.txt").decode("utf-8", errors="replace")
    expected = expected_sum(sums, asset)
    archive = fetcher(f"{base}/v{version}/{asset}")
    actual = hashlib.sha256(archive).hexdigest()
    if actual != expected:
        raise ShimError(f"sha256 mismatch for {asset}: expected {expected}, got {actual}; refusing to run it")
    files = extract(asset, archive, wanted)

    # Unpack next to the final directory, then rename: a concurrent first run
    # either wins the rename or finds the winner's files in place.
    directory.parent.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix=f".{tgt}-", dir=directory.parent))
    try:
        for file, content in files.items():
            p = tmp / file
            p.write_bytes(content)
            p.chmod(p.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        try:
            os.rename(tmp, directory)
        except OSError:
            if not binary.is_file():
                raise
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    log(f"sha256 verified ({actual}); cached in {directory}")
    return binary


# ---------------------------------------------------------------- run


def run(name: str, args: Optional[list] = None) -> int:
    args = sys.argv[1:] if args is None else args
    try:
        binary = ensure_binary(name)
    except ShimError as e:
        log(str(e))
        return 1
    if os.name == "posix":
        os.execv(str(binary), [str(binary), *args])  # does not return
    # Windows has no exec that replaces the process: run as a child and
    # relay its exit code; Ctrl-C reaches the child through the console.
    try:
        return subprocess.call([str(binary), *args])
    except KeyboardInterrupt:
        return 130


def main_causari() -> None:
    sys.exit(run("causari"))


def main_re() -> None:
    sys.exit(run("re"))
