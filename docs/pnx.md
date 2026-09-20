# PNX in Causari — Proof of Non-Exfiltration

`re proxy` sees every request an agent sends to its model provider. With
`--pnx` it is also a **PNX egress witness** (TACET profile `crovia.pnx.v1`,
specified at [croviatrust.com/registry/tacet/pnx](https://croviatrust.com/registry/tacet/pnx/)):
it fingerprints every outbound body, commits the fingerprints to a sparse
Merkle map and signs a **run sheet** carrying the root. Afterwards you can
prove, to anyone and offline, that a set of protected assets — keys,
customer data, source files — never shared a substring of 47 bytes or more
with that traffic; or, when one did, hand over a signed record of the
exposure. Neither the sheet nor the proof contains traffic bytes or asset
bytes.

Causari records what an agent did; PNX proves what it did not do.

## In five commands

```bash
re proxy --pnx                      # witness a session; Ctrl-C signs the sheet
re pnx list                         # runs of this repository, open and closed
re pnx sheet                        # the signed run sheet (public), latest run
re pnx prove --asset api_key=.env --assets-dir src/secret/ --asset-env OPENAI_API_KEY
re pnx verify .causari/pnx/<run>/proof.json --asset api_key=.env ...
```

Point the agent at the proxy as usual (`OPENAI_BASE_URL`,
`ANTHROPIC_BASE_URL`). The proxy prints the run id when it starts and the
root, the sheet path and the `re pnx prove` line when it stops.

`verify` exits **0** when the proof is valid and every asset is absent,
**1** when it is valid but an asset was present, undetectable or only
partially covered (or, with `--strict`, on any warning), **2** when the
proof is invalid or unverifiable. `prove --fail-on-present` uses the same
0 / 1 split. These are the exit codes of
[`tacet-pnx`](https://pypi.org/project/crovia-tacet/), and the asset flags
(`--asset LABEL=PATH`, `--assets-dir DIR`, `--asset-env VAR`) are the same,
so a command line ports between the two.

## What the proxy does in witness mode

Every request body is fingerprinted and **persisted before it is
forwarded**. If the witness cannot record a body (disk full, run directory
gone), the body is not forwarded and the client gets a 503 saying why. The
sheet's claim is "everything that left through here is in the map"; a hole
in the map could only turn a `present` into an `absent`, so the proxy fails
closed rather than open. The reverse failure — a crash between recording
and forwarding — leaves fingerprints of a body that never left, which can
only turn an `absent` into a `present`.

Empty bodies (GETs, health checks) carry nothing and are not counted. The
body is fingerprinted as it goes upstream, which for streamed OpenAI chat
requests includes the `stream_options.include_usage` field the proxy adds.

Two layers are fingerprinted, and the sheet declares both:

- the **raw bytes** of the body;
- **`json-strings-v1`**: every decoded string value of a JSON body that can
  hold at least one 32-byte k-gram. A file quoted inside an LLM request
  arrives with its newlines and quotes escaped, so its raw bytes never form
  a 47-byte run; the decoded strings restore the guarantee.

Ctrl-C ends a proxy session, so in witness mode it also signs the sheet: the
handler waits for any body being recorded at that moment, closes the run and
exits. A proxy killed without a chance to close (SIGKILL, power loss) leaves
the run open with its fingerprint log intact; `re pnx sheet --close` signs
it, and `re proxy --pnx --pnx-run-id <id>` resumes it with the recorded
salt.

## On disk

```text
.causari/pnx/<run_id>/
  meta.json          salt, parameters, counts, timestamps (rewritten per body)
  fingerprints.log   one salted fingerprint per line, appended before forwarding (0600)
  sheet.json         the signed run sheet, written once when the run is closed  — public
  proof.json         default output of `re pnx prove`                           — public
.causari/keys/pnx-witness.key   the witness key (0600), never the seal issuer key
```

Fingerprints are salted SHA-256 digests of 32-byte windows: nothing about
the traffic can be recovered from them. They do allow membership tests
against *guessed* strings once the salt is known, and the salt is in the
public sheet, so the log is owner-readable and `.causari/` stays
gitignored. Only `sheet.json` and a proof are meant to leave the machine.

The witness identity is `urn:crovia:pnx-witness:causari:<first 12 hex of
the public key>`, created on first use. A sheet and a Seal receipt are
different statements and use different keys.

## What a verified proof says — and does not

A valid proof with verdict `absent` says: *this witness, over the bodies it
saw between `first_at` and `last_at`, normalised as the sheet declares,
committed to a set of fingerprints of which none is a fingerprint of the
asset*. Because of the winnowing parameters (k-gram 32, window 16), any
shared substring of **47 bytes or more** is guaranteed to produce a common
fingerprint; the verdict is `absent` only for assets at least that long.

Per asset, the verdict is one of:

| verdict | meaning |
|---|---|
| `absent` | asset ≥ 47 bytes; no fingerprint in the map; guarantee holds |
| `absent-partial` | asset between 32 and 46 bytes; every k-gram hash checked and absent; a shared substring shorter than the whole asset may still have left |
| `present` | at least one fingerprint of the asset is in the map: a shared run of bytes of at least 32 bytes left through the proxy |
| `undetectable` | asset shorter than 32 bytes; nothing to fingerprint; never counted as clean |

The overall verdict is `present` if any asset is present, `absent` if
every asset is absent, otherwise `mixed`.

It says **nothing** about:

- bytes the witness did not see: traffic that bypassed the proxy, a second
  agent, TLS the proxy did not terminate, side channels;
- paraphrase, translation, summarisation, or encodings not normalised:
  base64, URL-encoding, UTF-16 and compression are outside the proof;
- assets shorter than 32 bytes;
- the honesty of the witness about what it ingested. A single-witness sheet
  is a statement by that witness. Multi-witness countersigning of the same
  egress removes the single point of trust.

When the asset bytes are **not** supplied to `verify`, the paths are checked
for the keys the proof lists, which proves non-inclusion of *those keys*
only; the result carries a warning saying so, and `--strict` turns it into
exit 1.

## Verification, offline

`re pnx verify` performs steps 1–4 of PNX.md §6:

1. the sheet: profile, parameter consistency, known normalisation layers,
   Ed25519 signature over the CSC-1 encoding of the sheet without its
   `signature` member, prefixed by `CROVIA-PNX-SHEET-v1\n`;
2. with asset bytes: `asset_sha256`, the detection class and the fingerprint
   set are recomputed and must match the proof;
3. every inclusion / non-inclusion path against `root` (depth-256 sparse
   Merkle map, TACET SPEC §5, compact encoding with a 256-bit bitmap);
4. every per-asset verdict and the overall verdict are recomputed and must
   match.

No network. A proof delivered inside a `crovia.seal.v1` by
`tacet-pnx prove --seal-key` is accepted as well: the outer Seal is verified
with the same code as `re seal verify`, and must bind the query (run id and
asset hashes) and the proof by hash. An invalid Seal fails the whole
verification.

## Interoperability with the Python reference

The Rust implementation is byte-identical to the reference in
`crovia-tacet` (`tacet/egress.py`, `tacet/smt.py`): same roots for the same
key set, same path encoding, same signed payload. Given the same key and
close time, `re` and `tacet-pnx` produce the same sheet and the same proof,
byte for byte.

- Conformance vectors under [`tests/vectors/pnx/`](../tests/vectors/pnx/)
  (fingerprints, map roots and paths, a signed sheet and four proofs) are
  generated by the reference with `generate.py` and checked in; CI asserts
  agreement on every platform without a Python install.
- When `tacet-pnx` is on `PATH`, [`tests/pnx_cli.rs`](../tests/pnx_cli.rs)
  additionally verifies proofs from `re pnx prove` with `tacet-pnx verify`
  and proofs from `tacet-pnx witness` + `prove` (bare and sealed) with
  `re pnx verify`, checking verdicts and exit codes in both directions.

## In CI

The [Causari Action](../action.yml) verifies a proof produced earlier in the
job and appends its verdict to the job summary and the sticky PR comment:

```yaml
- uses: croviatrust/causari@v1
  with:
    pnx-proof: pnx/proof.json          # written by `re pnx prove` or `tacet-pnx prove`
    pnx-assets: |                      # LABEL=PATH lines, or directories
      api_key=secrets/openai.txt
      protected/
    pnx-fail-on-present: "false"       # "true": fail unless every asset is absent
```

An invalid proof always fails the step. Asset bytes never enter the summary
or the comment. To produce the proof in the same job, run the agent behind
`re proxy --pnx` (stop it with `kill -INT` when the agent is done), then
`re pnx prove`; or use [`croviatrust/pnx-action`](https://github.com/croviatrust/pnx-action)
around any captured egress.
