'use strict';
// Thin launcher for the causari release binaries. On first run it downloads
// the archive for this platform from the GitHub release that matches the
// package version, checks its SHA-256 against the release's SHA256SUMS.txt,
// unpacks `causari` and `re` into a per-version cache and runs the requested
// one with the caller's arguments. Nothing happens at `npm install`: no
// network, no postinstall. Node built-ins only.
//
// Environment:
//   CAUSARI_VERSION        release to run (default: this package's version)
//   CAUSARI_BINARY         path to an existing binary; skips the download
//   CAUSARI_DOWNLOAD_BASE  where releases live (default: GitHub releases)
//   XDG_CACHE_HOME / LOCALAPPDATA  cache root (default: ~/.cache)

const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');
const https = require('https');
const zlib = require('zlib');
const crypto = require('crypto');
const { spawn } = require('child_process');

const REPO = 'croviatrust/causari';
const DEFAULT_BASE = `https://github.com/${REPO}/releases/download`;
const PACKAGE_VERSION = require('../package.json').version;
const MAX_REDIRECTS = 5;
const TIMEOUT_MS = 60_000;

class ShimError extends Error {}

function log(msg) {
  process.stderr.write(`causari: ${msg}\n`);
}

// ---------------------------------------------------------------- platform

function target(platform = process.platform, arch = process.arch) {
  const table = {
    'linux-x64': 'x86_64-unknown-linux-gnu',
    'linux-arm64': 'aarch64-unknown-linux-gnu',
    'darwin-x64': 'x86_64-apple-darwin',
    'darwin-arm64': 'aarch64-apple-darwin',
    'win32-x64': 'x86_64-pc-windows-msvc',
  };
  const t = table[`${platform}-${arch}`];
  if (!t) {
    throw new ShimError(
      `no prebuilt binary for ${platform}/${arch}. Releases cover Linux (x86_64, aarch64), ` +
        `macOS (x86_64, Apple silicon) and Windows (x86_64). Build from source instead: ` +
        `cargo install causari --locked`
    );
  }
  return t;
}

function version(env = process.env) {
  const v = (env.CAUSARI_VERSION || PACKAGE_VERSION).trim();
  return v.startsWith('v') ? v.slice(1) : v;
}

function assetName(ver, tgt) {
  const ext = tgt.endsWith('-windows-msvc') ? 'zip' : 'tar.gz';
  return `causari-v${ver}-${tgt}.${ext}`;
}

function cacheRoot(env = process.env, platform = process.platform) {
  if (env.XDG_CACHE_HOME) return env.XDG_CACHE_HOME;
  if (platform === 'win32' && env.LOCALAPPDATA) return env.LOCALAPPDATA;
  return path.join(os.homedir(), '.cache');
}

function cacheDir(ver, tgt, env = process.env, platform = process.platform) {
  return path.join(cacheRoot(env, platform), 'causari', ver, tgt);
}

function exeName(name, tgt) {
  return tgt.endsWith('-windows-msvc') ? `${name}.exe` : name;
}

// ---------------------------------------------------------------- download

function fetch(url, redirects = 0) {
  return new Promise((resolve, reject) => {
    const mod = url.startsWith('http://') ? http : https;
    const req = mod.get(
      url,
      { headers: { 'User-Agent': `causari-npm-shim/${PACKAGE_VERSION}`, Accept: '*/*' } },
      (res) => {
        const status = res.statusCode || 0;
        if ([301, 302, 303, 307, 308].includes(status) && res.headers.location) {
          res.resume();
          if (redirects >= MAX_REDIRECTS) {
            reject(new ShimError(`too many redirects fetching ${url}`));
            return;
          }
          resolve(fetch(new URL(res.headers.location, url).toString(), redirects + 1));
          return;
        }
        if (status !== 200) {
          res.resume();
          reject(new ShimError(`HTTP ${status} fetching ${url}`));
          return;
        }
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => resolve(Buffer.concat(chunks)));
        res.on('error', reject);
      }
    );
    req.setTimeout(TIMEOUT_MS, () => req.destroy(new ShimError(`timeout fetching ${url}`)));
    req.on('error', (e) => reject(e instanceof ShimError ? e : new ShimError(`${e.message} fetching ${url}`)));
  });
}

function sha256(buf) {
  return crypto.createHash('sha256').update(buf).digest('hex');
}

// One line per file, `<hex>  <name>` or `<hex> *<name>` (sha256sum format).
function expectedSum(sumsText, asset) {
  for (const raw of sumsText.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    const m = /^([0-9a-fA-F]{64})\s+\*?(.+)$/.exec(line);
    if (m && m[2].trim() === asset) return m[1].toLowerCase();
  }
  throw new ShimError(`no checksum for ${asset} in SHA256SUMS.txt; refusing to run an unverified download`);
}

// ---------------------------------------------------------------- tar

// Reads a POSIX ustar / pax / GNU tar stream and returns {name: Buffer} for
// the regular files whose base name is in `wanted`. Release archives hold
// the two binaries at the top level, but a directory prefix is tolerated.
function readTar(buf, wanted) {
  const out = {};
  let off = 0;
  let paxPath = null;
  let longName = null;
  while (off + 512 <= buf.length) {
    const header = buf.subarray(off, off + 512);
    if (header.every((b) => b === 0)) break;
    const field = (start, len) => {
      const s = header.subarray(start, start + len);
      const end = s.indexOf(0);
      return s.subarray(0, end === -1 ? len : end).toString('utf8');
    };
    const size = parseInt(field(124, 12).trim() || '0', 8);
    if (!Number.isFinite(size)) throw new ShimError('corrupt tar header');
    const type = String.fromCharCode(header[156] || 0x30);
    let name = field(0, 100);
    const prefix = field(257, 6) === 'ustar' ? field(345, 155) : '';
    if (prefix) name = `${prefix}/${name}`;
    const dataStart = off + 512;
    const data = buf.subarray(dataStart, dataStart + size);
    off = dataStart + Math.ceil(size / 512) * 512;

    if (type === 'x') {
      // pax extended header: "<len> path=<value>\n" records
      let p = 0;
      const text = data.toString('utf8');
      while (p < text.length) {
        const sp = text.indexOf(' ', p);
        if (sp === -1) break;
        const len = parseInt(text.slice(p, sp), 10);
        if (!len) break;
        const rec = text.slice(sp + 1, p + len - 1);
        const eq = rec.indexOf('=');
        if (eq !== -1 && rec.slice(0, eq) === 'path') paxPath = rec.slice(eq + 1);
        p += len;
      }
      continue;
    }
    if (type === 'L') {
      longName = data.toString('utf8').replace(/\0+$/, '');
      continue;
    }
    const effective = paxPath || longName || name;
    paxPath = null;
    longName = null;
    if (type !== '0' && type !== '\0') continue;
    const base = effective.split('/').filter(Boolean).pop();
    if (wanted.includes(base) && !(base in out)) out[base] = Buffer.from(data);
  }
  return out;
}

// ---------------------------------------------------------------- zip

// Reads a zip central directory and inflates the wanted entries (methods 0
// and 8, which is what 7z writes for the Windows release). Built in rather
// than shelling out to tar.exe or Expand-Archive: no dependency on PATH or
// the execution policy, and the same code path is testable on every OS.
function readZip(buf, wanted) {
  const out = {};
  let eocd = -1;
  for (let i = buf.length - 22; i >= Math.max(0, buf.length - 22 - 65_535); i--) {
    if (buf.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd === -1) throw new ShimError('corrupt zip: no end-of-central-directory record');
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  for (let i = 0; i < count; i++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new ShimError('corrupt zip: bad central directory entry');
    const method = buf.readUInt16LE(p + 10);
    const compSize = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extraLen = buf.readUInt16LE(p + 30);
    const commentLen = buf.readUInt16LE(p + 32);
    const localOff = buf.readUInt32LE(p + 42);
    const name = buf.subarray(p + 46, p + 46 + nameLen).toString('utf8');
    p += 46 + nameLen + extraLen + commentLen;
    const base = name.split('/').filter(Boolean).pop();
    if (!wanted.includes(base) || name.endsWith('/')) continue;
    if (buf.readUInt32LE(localOff) !== 0x04034b50) throw new ShimError('corrupt zip: bad local header');
    const lNameLen = buf.readUInt16LE(localOff + 26);
    const lExtraLen = buf.readUInt16LE(localOff + 28);
    const start = localOff + 30 + lNameLen + lExtraLen;
    const raw = buf.subarray(start, start + compSize);
    if (method === 0) out[base] = Buffer.from(raw);
    else if (method === 8) out[base] = zlib.inflateRawSync(raw);
    else throw new ShimError(`zip entry ${name} uses unsupported compression method ${method}`);
  }
  return out;
}

function extract(asset, buf, wanted) {
  const files = asset.endsWith('.zip') ? readZip(buf, wanted) : readTar(zlib.gunzipSync(buf), wanted);
  for (const w of wanted) {
    if (!files[w]) throw new ShimError(`archive ${asset} does not contain ${w}`);
  }
  return files;
}

// ---------------------------------------------------------------- install

async function ensureBinary(name, opts = {}) {
  const env = opts.env || process.env;
  if (env.CAUSARI_BINARY) {
    if (!fs.existsSync(env.CAUSARI_BINARY)) {
      throw new ShimError(`CAUSARI_BINARY=${env.CAUSARI_BINARY} does not exist`);
    }
    return env.CAUSARI_BINARY;
  }
  const ver = version(env);
  const tgt = target(opts.platform, opts.arch);
  const dir = cacheDir(ver, tgt, env, opts.platform);
  const wanted = ['causari', 're'].map((n) => exeName(n, tgt));
  const bin = path.join(dir, exeName(name, tgt));
  if (fs.existsSync(bin)) return bin;

  const base = (env.CAUSARI_DOWNLOAD_BASE || DEFAULT_BASE).replace(/\/+$/, '');
  const asset = assetName(ver, tgt);
  const fetcher = opts.fetch || fetch;
  log(`first run: downloading causari v${ver} (${tgt}) from ${base}/v${ver}/`);
  const sums = (await fetcher(`${base}/v${ver}/SHA256SUMS.txt`)).toString('utf8');
  const expected = expectedSum(sums, asset);
  const archive = await fetcher(`${base}/v${ver}/${asset}`);
  const actual = sha256(archive);
  if (actual !== expected) {
    throw new ShimError(`sha256 mismatch for ${asset}: expected ${expected}, got ${actual}; refusing to run it`);
  }
  const files = extract(asset, archive, wanted);

  // Unpack next to the final directory, then rename: a concurrent first run
  // either wins the rename or finds the winner's files in place.
  fs.mkdirSync(path.dirname(dir), { recursive: true });
  const tmp = fs.mkdtempSync(path.join(path.dirname(dir), `.${tgt}-`));
  try {
    for (const [file, data] of Object.entries(files)) {
      fs.writeFileSync(path.join(tmp, file), data, { mode: 0o755 });
    }
    try {
      fs.renameSync(tmp, dir);
    } catch (e) {
      if (!fs.existsSync(bin)) throw e;
    }
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
  log(`sha256 verified (${actual}); cached in ${dir}`);
  return bin;
}

// ---------------------------------------------------------------- run

function run(name, args = process.argv.slice(2)) {
  ensureBinary(name)
    .then((bin) => {
      const child = spawn(bin, args, { stdio: 'inherit', windowsHide: true });
      const forward = (sig) => {
        if (!child.killed) child.kill(sig);
      };
      for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
        process.on(sig, forward);
      }
      child.on('error', (e) => {
        log(`could not start ${bin}: ${e.message}`);
        process.exit(126);
      });
      child.on('exit', (code, signal) => {
        if (signal) {
          process.kill(process.pid, signal);
          return;
        }
        process.exit(code === null ? 1 : code);
      });
    })
    .catch((e) => {
      log(e instanceof ShimError ? e.message : `unexpected error: ${e && e.stack ? e.stack : e}`);
      process.exit(1);
    });
}

module.exports = {
  ShimError,
  target,
  version,
  assetName,
  cacheDir,
  fetch,
  sha256,
  expectedSum,
  readTar,
  readZip,
  extract,
  ensureBinary,
  run,
};
