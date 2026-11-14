# Backport the ParityDB log-cleanup deadlock fix

This copy of `parity-db 0.4.13` applies upstream
[PR #252](https://github.com/paritytech/parity-db/pull/252), released in 0.5.6.
When log enactment exceeds the cleanup cap, the commit worker must wake the
cleanup worker before waiting. The wait must also stop during shutdown.
Without these changes, sustained writes can stop block import indefinitely.

## SDK and storage compatibility

The locked SDK commit is
[`4fd8081b96595ff1341ba4e7754a21dd8a28bfb8`](https://github.com/duniter/duniter-polkadot-sdk/tree/4fd8081b96595ff1341ba4e7754a21dd8a28bfb8).
Its `sc-client-db` requires `parity-db ^0.4.12`. Cargo rejects
`cargo update -p parity-db --precise 0.5.6` because 0.5.6 does not satisfy that
requirement. Adding a separate 0.5.6 dependency would leave the SDK using 0.4.13.
The root `[patch.crates-io]` instead replaces the SDK's existing dependency.

This patch retains version 0.4.13, its public API, dependencies, column options,
WAL encoding, and database format version 8. It requires no database migration
or resynchronization. It preserves the existing `sync_wal` and `sync_data`
defaults. Version 0.5.6 includes multitree support and uses database format 9 for
new databases; taking that release would require a separate SDK dependency and
compatibility review.

The compatibility test links both the original crates.io release and this copy.
It creates a database with the original, reads and updates it with the backport,
then reopens and writes with the original. It checks multipart values, deletion,
and unchanged metadata. These checks cover upgrading and rolling back this patch.
The fix addresses a deadlock, not a demonstrated data-corruption defect.

## Source and maintenance

Source files, upstream tests, documentation, and license texts come from the
[crates.io archive of parity-db 0.4.13](https://static.crates.io/crates/parity-db/parity-db-0.4.13.crate),
SHA-256 `592a28a24b09c9dc20ac8afaa6839abc417c720afe42c12e1e4a9d6aa2508d2e`.
Its recorded upstream commit is
[`8b5dba861dd787dd63aaeffe44c1061d1e6bb80f`](https://github.com/paritytech/parity-db/tree/8b5dba861dd787dd63aaeffe44c1061d1e6bb80f).

Changes from that release:

- Apply PR #252 to `src/db.rs`, without changing the upstream fix.
- Register `src/log_cleanup_tests.rs` for tests outside Loom.
- Add four regression tests covering cleanup wakeup and shutdown with both
  `sync_data` settings. Without the fix, all four fail after a bounded timeout.
- Use `Cargo.toml.orig` with a standalone workspace that omits the unpublished
  `admin` member. Dependency requirements remain unchanged.
- Add a separate compatibility-test workspace and a bounded block-load example.

Standalone lockfiles are ignored. Production uses the root `Cargo.lock`.
Remove this copy and the root patch when the SDK accepts an upstream ParityDB
release containing #252. Review the storage format and rollback requirements
before moving to the 0.5 series.

## Run the checks

From the repository root, prepare the SDK first:

```sh
./scripts/prepare_local_polkadot_sdk.sh Cargo.lock duniter-polkadot-sdk.git
CARGO_TARGET_DIR=target/parity-db ./scripts/cargo_with_vendor.sh test --manifest-path patches/parity-db/Cargo.toml
CARGO_TARGET_DIR=target/parity-db ./scripts/cargo_with_vendor.sh test --manifest-path patches/parity-db/compatibility-tests/Cargo.toml
```

The `tests_parity_db` CI job runs both suites. Run the full node checks from
`.gitlab-ci.yml` as well, including `fmt_and_clippy` and `tests_cucumber`.

The load example submits 512 block batches, each with 256 scattered keys and
1 KiB values, for 128 MiB of value payload. Each block reads 16 earlier state
entries. Timing includes draining the queue and flushing on close. Reopening
checks one persisted value per block. This is a bounded storage workload, not
a full Duniter block-execution benchmark or an HDD saturation test.

```sh
CARGO_TARGET_DIR=target/parity-db ./scripts/cargo_with_vendor.sh build --release --manifest-path patches/parity-db/compatibility-tests/Cargo.toml --example block_load
target/parity-db/release/examples/block_load original
target/parity-db/release/examples/block_load patched
```
