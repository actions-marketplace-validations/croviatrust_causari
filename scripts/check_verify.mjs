#!/usr/bin/env node
// The causari.dev/verify page, driven from Node: site/verify/seal-core.js and
// site/verify/verify.js are loaded as plain scripts into a bare context with
// WebCrypto and nothing else (no DOM, no network), exactly as a browser runs
// them, and the verifier is exercised against a real bundle from
// `re audit --seal` plus every alteration the CLI is tested against.
//
//   node scripts/check_verify.mjs audit.seal.json [seals.jsonl]
//
// Exit 0 when every case gives the expected verdict, 1 otherwise, 2 when
// this Node cannot verify Ed25519 (WebCrypto Ed25519 needs Node 18.4+).
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createContext, runInContext } from "node:vm";
import { webcrypto } from "node:crypto";

const here = dirname(fileURLToPath(import.meta.url));
const site = join(here, "..", "site", "verify");
const [bundlePath, chainPath] = process.argv.slice(2);
if (!bundlePath) {
  console.error("usage: check_verify.mjs audit.seal.json [seals.jsonl]");
  process.exit(2);
}

const ctx = createContext({ crypto: webcrypto, TextEncoder, TextDecoder, console });
for (const f of ["seal-core.js", "verify.js"]) {
  runInContext(readFileSync(join(site, f), "utf8"), ctx, { filename: f });
}
const { causariVerify, ed25519Supported } = ctx;
if (!(await runInContext("ed25519Supported()", ctx))) {
  console.error("this Node has no WebCrypto Ed25519; cannot drive the page");
  process.exit(2);
}
void ed25519Supported;

const verify = (text) => causariVerify.verifyDocument(text, []);
const bundleText = readFileSync(bundlePath, "utf8");
const bundle = JSON.parse(bundleText);
const clone = () => JSON.parse(bundleText);
const show = (v) => JSON.stringify(v, null, 2);

let failures = 0;
async function expect(name, text, want) {
  const r = await verify(text);
  const ok = r.valid === want.valid && r.kind === want.kind && (!want.reason || (r.reason || "").includes(want.reason));
  const line = `${ok ? "ok  " : "FAIL"} ${name.padEnd(46)} → ${r.valid ? "valid" : r.kind === "unreadable" ? "unreadable" : "invalid"} (${r.kind})${r.reason ? ": " + r.reason : ""}`;
  console.log(line);
  if (!ok) {
    failures++;
    console.log(`     expected ${JSON.stringify(want)}`);
    const last = r.steps.filter((s) => !s.hdr).slice(-3);
    for (const s of last) console.log(`     ${s.ok ? "✓" : "✗"} ${s.label} ${s.det}`);
  }
  return r;
}

// The bundle as written by the CLI: valid, and the statement is the seal's.
const good = await expect("bundle as written", bundleText, { valid: true, kind: "audit" });
if (good.valid) {
  const s = good.statement;
  const p = bundle.seal.generator.params;
  const same = s.commit === p.commit && s.method === p.method && s.repo === p.repo && s.seal_id === bundle.seal.seal_id && String(s.shallow) === p["coverage.shallow"];
  console.log(`${same ? "ok  " : "FAIL"} statement matches the signed params`);
  if (!same) { failures++; console.log(show(s)); }
}
await expect("bundle, compact JSON", JSON.stringify(bundle), { valid: true, kind: "audit" });

// Numbers edited: output hash mismatch.
{
  const b = clone();
  const audit = JSON.parse(b.subject.audit_json);
  audit.verified.surviving += 1;
  b.subject.audit_json = JSON.stringify(audit, null, 2) + "\n";
  await expect("audit JSON numbers altered", show(b), { valid: false, kind: "audit", reason: "numbers were altered" });
}
// Same, with the hash "fixed" to the new bytes: the signature catches it.
{
  const b = clone();
  const altered = b.subject.audit_json.replace(/"total_commits": \d+/, '"total_commits": 999');
  const digest = await webcrypto.subtle.digest("SHA-256", new TextEncoder().encode(altered));
  b.seal.subject.output_hash = "sha256:" + [...new Uint8Array(digest)].map((x) => x.toString(16).padStart(2, "0")).join("");
  b.seal.subject.output_len = new TextEncoder().encode(altered).length;
  b.subject.audit_json = altered;
  await expect("numbers altered, hash re-fitted", show(b), { valid: false, kind: "audit", reason: "signature" });
}
// Wrong commit in the input: input hash mismatch.
{
  const b = clone();
  b.subject.input.commit = "0".repeat(40);
  await expect("commit swapped in subject.input", show(b), { valid: false, kind: "audit", reason: "input_hash" });
}
// Wrong commit in the signed params: signature fails.
{
  const b = clone();
  b.seal.generator.params.commit = "0".repeat(40);
  await expect("commit swapped in signed params", show(b), { valid: false, kind: "audit", reason: "signature" });
}
// Method swapped in the signed params.
{
  const b = clone();
  b.seal.generator.params.method = "v9";
  await expect("method swapped in signed params", show(b), { valid: false, kind: "audit", reason: "signature" });
}
// Method swapped in the input only.
{
  const b = clone();
  b.subject.input.method = "v1";
  await expect("method swapped in subject.input", show(b), { valid: false, kind: "audit", reason: "input_hash" });
}
// Unknown fields anywhere in the bundle: fail-closed.
{
  const b = clone();
  b.note = "SOC2 certified";
  await expect("unknown top-level field", show(b), { valid: false, kind: "audit", reason: "unknown field" });
}
{
  const b = clone();
  b.subject.claim = "trust me";
  await expect("unknown subject field", show(b), { valid: false, kind: "audit", reason: "unknown field" });
}
{
  const b = clone();
  b.seal.badge = "gold";
  await expect("unknown seal field", show(b), { valid: false, kind: "audit", reason: "unknown" });
}
// Signature bit-flipped.
{
  const b = clone();
  const sig = b.seal.signature.sig_hex;
  b.seal.signature.sig_hex = (sig[0] === "0" ? "1" : "0") + sig.slice(1);
  await expect("signature bit flipped", show(b), { valid: false, kind: "audit", reason: "signature" });
}
// Another issuer's key on the same seal.
{
  const b = clone();
  b.seal.issuer.pubkey.key_hex = "1".repeat(64);
  await expect("issuer key replaced", show(b), { valid: false, kind: "audit" });
}
// Shallow contradiction: params say full history over a shallow audit.
{
  const b = clone();
  b.subject.audit_json = b.subject.audit_json.replace('"shallow": false', '"shallow": true');
  await expect("shallow flag altered in audit JSON", show(b), { valid: false, kind: "audit", reason: "numbers were altered" });
}
// Duplicate key: JSON.parse would keep the second audit_json; the page refuses.
{
  const dup = bundleText.replace(/"audit_json": /, '"audit_json": "{}", "audit_json": ');
  await expect("duplicate key audit_json", dup, { valid: false, kind: "unreadable", reason: "duplicate key" });
}
await expect("not JSON", "not json", { valid: false, kind: "unreadable" });
await expect("empty", "   \n", { valid: false, kind: "unreadable" });
await expect("a JSON array of nothing", "[]", { valid: false, kind: "unreadable" });
await expect("bundle kind renamed", show({ ...clone(), bundle: "something.else" }), { valid: false, kind: "seal" });

// The bare seal inside the bundle verifies on its own; a tampered one does not.
await expect("bare seal from the bundle", show(bundle.seal), { valid: true, kind: "seal" });
{
  const s = clone().seal;
  s.chain.sequence = 7;
  await expect("bare seal, sequence edited", show(s), { valid: false, kind: "seal", reason: "signature" });
}
{
  const s = clone().seal;
  s.generator.params.repo = "https://github.com/someone/else";
  await expect("bare seal, repo edited", show(s), { valid: false, kind: "seal", reason: "signature" });
}

// The chain, when a seals.jsonl is given.
if (chainPath) {
  const chainText = readFileSync(chainPath, "utf8");
  const seals = chainText.split(/\r?\n/).filter(Boolean).map((l) => JSON.parse(l));
  const r = await expect(`seals.jsonl chain (${seals.length} seals)`, chainText, { valid: true, kind: "chain" });
  if (r.valid && r.statement.seals !== seals.length) { failures++; console.log("FAIL chain length " + r.statement.seals); }
  await expect("chain as a JSON array", show(seals), { valid: true, kind: "chain" });
  const jsonl = (list) => list.map((s) => JSON.stringify(s)).join("\n") + "\n";
  if (seals.length < 3) {
    failures++;
    console.log("FAIL the chain needs at least three seals to test gaps and fragments");
  } else {
    await expect("chain with the middle seal removed", jsonl(seals.filter((_, i) => i !== 1)), { valid: false, kind: "chain", reason: "chain gap or fork" });
    const forked = seals.map((s) => JSON.parse(JSON.stringify(s)));
    forked[1].chain.prev_seal_hash = "sha256:" + "ab".repeat(32);
    await expect("chain link rewritten (signature breaks first)", jsonl(forked), { valid: false, kind: "chain", reason: "signature" });
    await expect("chain reordered", jsonl([seals[1], seals[0], ...seals.slice(2)]), { valid: false, kind: "chain", reason: "chain gap or fork" });
    await expect("chain fragment (no genesis)", jsonl(seals.slice(1)), { valid: true, kind: "chain" });
    const gapped = JSON.parse(JSON.stringify(seals[0]));
    gapped.chain.sequence = 1;
    await expect("genesis with a non-zero sequence", jsonl([gapped, ...seals.slice(1)]), { valid: false, kind: "chain", reason: "signature" });
    const notGenesis = JSON.parse(JSON.stringify(seals[0]));
    notGenesis.chain.prev_seal_hash = "sha256:" + "cd".repeat(32);
    await expect("genesis with a prev hash", jsonl([notGenesis, ...seals.slice(1)]), { valid: false, kind: "chain", reason: "signature" });
  }
  await expect("one line of the chain", jsonl([seals[0]]), { valid: true, kind: "seal" });
}

console.log(failures ? `\n${failures} case(s) disagree with the CLI` : "\nevery case agrees with the CLI");
process.exit(failures ? 1 : 0);
