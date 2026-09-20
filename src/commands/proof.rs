//! `re proof` is retired.
//!
//! It signed a summary of the local ledger with its own envelope format
//! (`causari.proof.v0.2`). The audit result is now a Crovia Seal
//! (`crovia.seal.v1`, `re audit --seal`), bound to the commit and the
//! method version, verifiable with `re seal verify` or at causari.dev/verify
//! by the same code path as every other seal. One receipt format, one
//! verifier.

use anyhow::{Result, anyhow};

use crate::cli::ProofArgs;
use crate::exit::exit_with;

pub fn run(_args: ProofArgs) -> Result<()> {
    Err(exit_with(
        2,
        anyhow!(
            "`re proof` has been retired.\n  \
             Issue:  re audit --seal            (writes audit.seal.json, a crovia.seal.v1 over the audit)\n  \
             Verify: re seal verify audit.seal.json   or   https://causari.dev/verify\n  \
             Old causari-proof.json files are not verifiable by this release."
        ),
    ))
}
