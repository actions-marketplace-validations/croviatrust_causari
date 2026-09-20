#!/usr/bin/env python3
"""Regenerate the PNX conformance vectors from the Python reference.

    python3 tests/vectors/pnx/generate.py

Needs the `crovia-tacet` reference importable (`pip install crovia-tacet`,
or PYTHONPATH pointing at tacet/reference/python). The vectors are what
`src/pnx.rs` is tested against in CI, where the reference is not installed;
the live cross-check tests in the same module rerun the reference when it is.

Everything here is deterministic: fixed salt, fixed witness seed, fixed PRNG
seeds, fixed timestamps. Running it twice yields identical files.
"""
from __future__ import annotations

import json
import random
from pathlib import Path

from tacet import egress
from tacet.hashing import EMPTY, DEPTH
from tacet.keys import SigningKey
from tacet.smt import SparseMerkleMap

HERE = Path(__file__).resolve().parent
SALT = bytes([0x01]) * 16
SEED = bytes([0x07]) * 32
WITNESS_ID = "urn:crovia:pnx-witness:vectors"


def blob(data: bytes) -> dict:
    """A byte string as JSON: UTF-8 text when it is one, hex otherwise."""
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return {"hex": data.hex()}
    if text.encode("utf-8") != data:
        return {"hex": data.hex()}
    return {"text": text}


def rand_bytes(rng: random.Random, n: int) -> bytes:
    return bytes(rng.getrandbits(8) for _ in range(n))


def dump(name: str, obj: dict) -> None:
    (HERE / name).write_text(json.dumps(obj, indent=1, ensure_ascii=False, sort_keys=False) + "\n")


# --------------------------------------------------------------------------- bodies shared by the vectors

ENV_FILE = "OPENAI_API_KEY=sk-live-0123456789abcdef0123456789abcdef\nDATABASE_URL=postgres://app:s3cr3t@db.internal:5432/prod\n"
SOURCE = "fn refresh_token(user: &User) -> Result<Token> {\n    rotate_every(Duration::hours(24))\n}\n"


def bodies() -> list[tuple[str, bytes]]:
    rng = random.Random(2026)
    chat = json.dumps({
        "model": "gpt-4o",
        "stream": True,
        "messages": [
            {"role": "system", "content": "You are a careful assistant that reviews code and never reveals secrets."},
            {"role": "user", "content": "Here is my .env, is anything wrong?\n" + ENV_FILE},
        ],
    }, ensure_ascii=False).encode("utf-8")
    anthropic = json.dumps({
        "model": "claude-sonnet-4",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "Explain this function:\n```rust\n" + SOURCE + "```"},
        ]}],
    }).encode("utf-8")
    return [
        ("chat-with-env", chat),
        ("anthropic-with-source", anthropic),
        ("health", b"GET /health"),
        ("binary", rand_bytes(rng, 700)),
        ("exactly-threshold", rand_bytes(rng, egress.THRESHOLD)),
        ("one-kgram", rand_bytes(rng, egress.K_GRAM)),
        ("too-short", b"short body"),
        ("unicode-json", json.dumps({"q": "héllo wörld — ünïcödé strings count their UTF-8 bytes, not code points"}, ensure_ascii=False).encode("utf-8")),
        ("not-json-braces", b"{not json at all but long enough to have plenty of k-grams in it}"),
    ]


def assets() -> list[tuple[str, bytes]]:
    rng = random.Random(99)
    return [
        ("env-file", ENV_FILE.encode("utf-8")),
        ("source-file", SOURCE.encode("utf-8")),
        ("openai-key", b"sk-live-0123456789abcdef0123456789abcdef"),          # 40 bytes: partial, present
        ("safe-key", b"AKIA" + rand_bytes(rng, 60)),                             # absent
        ("mid", b"x" * 40),                                                      # partial, absent
        ("pin", b"1234"),                                                        # undetectable
        ("binary-secret", rand_bytes(rng, 80)),                                  # absent
    ]


# --------------------------------------------------------------------------- vectors

def gen_fingerprints() -> None:
    out = {"profile": egress.PROFILE, "salt_hex": SALT.hex(), "k_gram": egress.K_GRAM, "window": egress.WINDOW,
           "threshold": egress.THRESHOLD, "bodies": [], "assets": []}
    for name, body in bodies():
        kg = egress.kgram_hashes(body, SALT)
        out["bodies"].append({
            "name": name,
            "body": blob(body),
            "kgram_count": len(kg),
            **({"first_kgram_hash": kg[0].hex()} if kg else {}),
            "raw_fingerprints": sorted(fp.hex() for fp in egress.fingerprints(body, SALT)),
            "json_strings": [blob(s) for s in egress.json_strings(body)],
        })
    for name, asset in assets():
        klass, fps = egress.asset_fingerprints(asset, SALT)
        out["assets"].append({"name": name, "asset": blob(asset), "detection": klass,
                              "fingerprints": [fp.hex() for fp in fps]})
    dump("fingerprints.json", out)


def gen_smt() -> None:
    rng = random.Random(4242)
    out = {"empty_root": EMPTY[DEPTH].hex(), "present_leaf_value": egress.PRESENT.hex(), "cases": []}
    for name, n in (("one-key", 1), ("two-keys", 2), ("five-keys", 5), ("thirty-keys", 30)):
        keys = [rand_bytes(rng, 32) for _ in range(n)]
        m = SparseMerkleMap({k: egress.PRESENT for k in keys})
        paths = []
        for k in keys[:3]:
            paths.append({"key": k.hex(), "present": True, "path": m.prove(k).to_json()})
        for _ in range(2):
            absent = rand_bytes(rng, 32)
            paths.append({"key": absent.hex(), "present": False, "path": m.prove(absent).to_json()})
        # a key sharing a long prefix with a present key: deep divergence
        near = bytearray(keys[0]); near[-1] ^= 0x01
        paths.append({"key": bytes(near).hex(), "present": False, "path": m.prove(bytes(near)).to_json()})
        out["cases"].append({"name": name, "keys": [k.hex() for k in keys], "root": m.root().hex(), "paths": paths})
    dump("smt.json", out)


def gen_witness() -> None:
    w = egress.EgressWitness(run_id="vectors/run-1", salt=SALT)
    recorded = []
    for i, (name, body) in enumerate(bodies()):
        at = f"2026-09-20T10:00:{i:02d}Z"
        w.ingest(body, at)
        recorded.append({"name": name, "at": at, "body": blob(body)})
    key = SigningKey.from_seed(WITNESS_ID, SEED)
    sheet = w.sheet(key, "2026-09-20T10:05:00Z")
    all_assets = dict(assets())
    proofs = []
    for name, labels in (
        ("mixed", ["env-file", "source-file", "openai-key", "safe-key", "mid", "pin", "binary-secret"]),
        ("clean", ["safe-key", "binary-secret"]),
        ("exposed", ["env-file"]),
        ("partial-only", ["mid"]),
    ):
        chosen = [(l, all_assets[l]) for l in labels]
        proof = w.prove(sheet, chosen)
        res = egress.verify_pnx(proof, dict(chosen))
        assert res.ok, res.errors
        proofs.append({"name": name, "assets": {l: blob(b) for l, b in chosen}, "verdict": proof["verdict"],
                       "asset_verdicts": {a["label"]: a["verdict"] for a in proof["assets"]}, "proof": proof})
    dump("witness.json", {
        "profile": egress.PROFILE, "run_id": w.run_id, "salt_hex": SALT.hex(), "witness_seed_hex": SEED.hex(),
        "bodies": recorded, "root": sheet["root"], "fingerprints": len(w._map), "sheet": sheet, "proofs": proofs,
    })


if __name__ == "__main__":
    gen_fingerprints()
    gen_smt()
    gen_witness()
    print("wrote", ", ".join(p.name for p in sorted(HERE.glob("*.json"))))
