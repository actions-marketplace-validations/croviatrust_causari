"""Unit tests for the PyPI launcher. No network: every download goes through
a fake fetcher that serves an in-memory release."""

from __future__ import annotations

import hashlib
import io
import os
import stat
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import causari_shim as shim  # noqa: E402

VERSION = "9.9.9"
LINUX = "x86_64-unknown-linux-gnu"
WINDOWS = "x86_64-pc-windows-msvc"


def fake_tar(files: dict) -> bytes:
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tf:
        for name, content in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(content)
            info.mode = 0o755
            tf.addfile(info, io.BytesIO(content))
    return buf.getvalue()


def fake_zip(files: dict) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for name, content in files.items():
            zf.writestr(name, content)
    return buf.getvalue()


class Release:
    """An in-memory release directory served by ``fetch``."""

    def __init__(self, asset: str, archive: bytes, sums: str | None = None) -> None:
        self.asset = asset
        self.archive = archive
        self.sums = sums if sums is not None else f"{hashlib.sha256(archive).hexdigest()}  {asset}\n"
        self.requests: list = []

    def fetch(self, url: str) -> bytes:
        self.requests.append(url)
        if url.endswith("/SHA256SUMS.txt"):
            return self.sums.encode()
        if url.endswith("/" + self.asset):
            return self.archive
        raise shim.ShimError(f"HTTP 404 fetching {url}")


def refuse(url: str) -> bytes:
    raise AssertionError(f"network access attempted: {url}")


class ShimTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.env = {"XDG_CACHE_HOME": self.tmp.name, "CAUSARI_VERSION": VERSION}
        self.linux_asset = shim.asset_name(VERSION, LINUX)
        self.linux_bins = {"causari": b"#!/bin/sh\necho causari\n", "re": b"#!/bin/sh\necho re\n"}
        self.logged: list = []
        self._log = shim.log
        shim.log = self.logged.append

    def tearDown(self) -> None:
        shim.log = self._log
        self.tmp.cleanup()

    def cache(self, tgt: str) -> Path:
        return Path(self.tmp.name) / "causari" / VERSION / tgt

    # --- platform ------------------------------------------------------

    def test_targets(self) -> None:
        self.assertEqual(shim.target("Linux", "x86_64"), LINUX)
        self.assertEqual(shim.target("Linux", "aarch64"), "aarch64-unknown-linux-gnu")
        self.assertEqual(shim.target("Darwin", "arm64"), "aarch64-apple-darwin")
        self.assertEqual(shim.target("Darwin", "x86_64"), "x86_64-apple-darwin")
        self.assertEqual(shim.target("Windows", "AMD64"), WINDOWS)

    def test_unsupported_platform_names_itself_and_the_source_build(self) -> None:
        with self.assertRaises(shim.ShimError) as cm:
            shim.target("FreeBSD", "x86_64")
        self.assertIn("freebsd/x86_64", str(cm.exception))
        self.assertIn("cargo install causari --locked", str(cm.exception))
        with self.assertRaises(shim.ShimError):
            shim.target("Windows", "ARM64")

    def test_version_env_strips_v(self) -> None:
        self.assertEqual(shim.resolve_version({"CAUSARI_VERSION": "v1.2.3"}), "1.2.3")
        self.assertEqual(shim.resolve_version({"CAUSARI_VERSION": "1.2.3"}), "1.2.3")
        self.assertRegex(shim.resolve_version({}), r"^\d+\.\d+\.\d+$")

    def test_asset_names(self) -> None:
        self.assertEqual(shim.asset_name("0.2.0", LINUX), "causari-v0.2.0-x86_64-unknown-linux-gnu.tar.gz")
        self.assertEqual(shim.asset_name("0.2.0", WINDOWS), "causari-v0.2.0-x86_64-pc-windows-msvc.zip")

    def test_cache_root(self) -> None:
        self.assertEqual(shim.cache_root({"XDG_CACHE_HOME": "/x"}), Path("/x"))
        self.assertEqual(shim.cache_root({"LOCALAPPDATA": r"C:\L"}, "Windows"), Path(r"C:\L"))
        self.assertEqual(shim.cache_root({"LOCALAPPDATA": r"C:\L"}, "Linux"), Path.home() / ".cache")

    # --- checksums ------------------------------------------------------

    def test_expected_sum_parses_sha256sum_formats(self) -> None:
        h = "a" * 64
        self.assertEqual(shim.expected_sum(f"{h}  x.tar.gz\n", "x.tar.gz"), h)
        self.assertEqual(shim.expected_sum(f"{h} *x.tar.gz\r\n", "x.tar.gz"), h)
        self.assertEqual(shim.expected_sum(f"{'b' * 64}  y.tar.gz\n{h}  x.tar.gz\n", "x.tar.gz"), h)
        with self.assertRaises(shim.ShimError) as cm:
            shim.expected_sum(f"{h}  y.tar.gz\n", "x.tar.gz")
        self.assertIn("no checksum for x.tar.gz", str(cm.exception))

    def test_checksum_mismatch_refuses_and_caches_nothing(self) -> None:
        archive = fake_tar(self.linux_bins)
        tampered = Release(self.linux_asset, archive, sums=f"{'0' * 64}  {self.linux_asset}\n")
        with self.assertRaises(shim.ShimError) as cm:
            shim.ensure_binary("re", self.env, tampered.fetch, "Linux", "x86_64")
        self.assertIn("sha256 mismatch", str(cm.exception))
        self.assertFalse(self.cache(LINUX).exists())
        self.assertFalse(any(p.is_dir() for p in self.cache(LINUX).parent.glob(".*")) if self.cache(LINUX).parent.exists() else False)

    def test_missing_asset_in_sums_refuses(self) -> None:
        archive = fake_tar(self.linux_bins)
        rel = Release(self.linux_asset, archive, sums=f"{'0' * 64}  other.tar.gz\n")
        with self.assertRaises(shim.ShimError) as cm:
            shim.ensure_binary("re", self.env, rel.fetch, "Linux", "x86_64")
        self.assertIn("no checksum for", str(cm.exception))
        self.assertEqual(len(rel.requests), 1, "the archive is not fetched without a checksum to hold it to")

    # --- install --------------------------------------------------------

    def test_first_run_downloads_verifies_and_caches_both_names(self) -> None:
        rel = Release(self.linux_asset, fake_tar(self.linux_bins))
        binary = shim.ensure_binary("re", self.env, rel.fetch, "Linux", "x86_64")
        self.assertEqual(binary, self.cache(LINUX) / "re")
        self.assertEqual(binary.read_bytes(), self.linux_bins["re"])
        self.assertEqual((self.cache(LINUX) / "causari").read_bytes(), self.linux_bins["causari"])
        self.assertTrue(binary.stat().st_mode & stat.S_IXUSR)
        self.assertEqual(
            rel.requests,
            [
                f"{shim.DEFAULT_BASE}/v{VERSION}/SHA256SUMS.txt",
                f"{shim.DEFAULT_BASE}/v{VERSION}/{self.linux_asset}",
            ],
        )
        self.assertEqual([p.name for p in self.cache(LINUX).parent.iterdir()], [LINUX], "no temp dir left behind")
        self.assertTrue(any(m.startswith("first run: downloading") for m in self.logged), self.logged)
        self.assertTrue(any(m.startswith("sha256 verified") for m in self.logged), self.logged)

    def test_cache_hit_makes_no_request(self) -> None:
        d = self.cache(LINUX)
        d.mkdir(parents=True)
        (d / "causari").write_bytes(b"cached")
        binary = shim.ensure_binary("causari", self.env, refuse, "Linux", "x86_64")
        self.assertEqual(binary, d / "causari")

    def test_download_base_override(self) -> None:
        env = dict(self.env, CAUSARI_DOWNLOAD_BASE="https://mirror.example/causari/")
        rel = Release(self.linux_asset, fake_tar(self.linux_bins))
        shim.ensure_binary("re", env, rel.fetch, "Linux", "x86_64")
        self.assertTrue(all(u.startswith(f"https://mirror.example/causari/v{VERSION}/") for u in rel.requests), rel.requests)

    def test_windows_zip_yields_exe_names(self) -> None:
        asset = shim.asset_name(VERSION, WINDOWS)
        rel = Release(asset, fake_zip({"causari.exe": b"MZcausari", "re.exe": b"MZre"}))
        binary = shim.ensure_binary("re", self.env, rel.fetch, "Windows", "AMD64")
        self.assertEqual(binary, self.cache(WINDOWS) / "re.exe")
        self.assertEqual(binary.read_bytes(), b"MZre")

    def test_archive_missing_a_binary_is_refused(self) -> None:
        rel = Release(self.linux_asset, fake_tar({"re": b"only re"}))
        with self.assertRaises(shim.ShimError) as cm:
            shim.ensure_binary("re", self.env, rel.fetch, "Linux", "x86_64")
        self.assertIn("does not contain causari", str(cm.exception))
        self.assertFalse(self.cache(LINUX).exists())

    def test_archive_with_directory_prefix_is_accepted(self) -> None:
        rel = Release(self.linux_asset, fake_tar({"causari-v9/causari": b"c", "causari-v9/re": b"r"}))
        binary = shim.ensure_binary("re", self.env, rel.fetch, "Linux", "x86_64")
        self.assertEqual(binary.read_bytes(), b"r")

    def test_causari_binary_env_skips_download(self) -> None:
        own = Path(self.tmp.name) / "my-re"
        own.write_bytes(b"mine")
        env = dict(self.env, CAUSARI_BINARY=str(own))
        self.assertEqual(shim.ensure_binary("re", env, refuse, "Linux", "x86_64"), own)
        env["CAUSARI_BINARY"] = str(Path(self.tmp.name) / "missing")
        with self.assertRaises(shim.ShimError) as cm:
            shim.ensure_binary("re", env, refuse, "Linux", "x86_64")
        self.assertIn("does not exist", str(cm.exception))

    # --- run ------------------------------------------------------------

    def test_run_reports_shim_errors_with_exit_1(self) -> None:
        env = dict(self.env, CAUSARI_BINARY=str(Path(self.tmp.name) / "missing"))
        old = dict(os.environ)
        os.environ.update(env)
        try:
            self.assertEqual(shim.run("re", ["--version"]), 1)
        finally:
            os.environ.clear()
            os.environ.update(old)


if __name__ == "__main__":
    unittest.main()
