# ic-auction

[![crates.io](https://img.shields.io/crates/v/ic-auction.svg)](https://crates.io/crates/ic-auction)
[![docs.rs](https://docs.rs/ic-auction/badge.svg)](https://docs.rs/ic-auction)
[![CI](https://github.com/DrJesseGlass/ic-auction/actions/workflows/ci.yml/badge.svg)](https://github.com/DrJesseGlass/ic-auction/actions/workflows/ci.yml)

Allocation mechanisms for scarce names and slots on the Internet
Computer, as pure state machines a canister wraps.

- **harberger**: the holder self-assesses a price and prepays a balance;
  tax accrues on the price at a configured rate and settles lazily on
  every read and write; when the balance runs out a grace period starts,
  then the thing is free. Anyone may buy at the assessed price at any
  time, which is what keeps the price honest. No timers.
- **vickrey**: sealed-bid second-price auction. Bidders commit a hash of
  bidder, amount and salt with an escrowed deposit, reveal in the next
  phase, and the highest revealed bid wins at the second-highest price
  (or the reserve). Unrevealed commitments forfeit their deposit.
- **ledger** (feature `icrc`): the ICRC-1/ICRC-2 client for escrow and
  payout in cycles or any ICRC token: pull under an approval, pay, fund
  the canister, read the fee.

The mechanisms hold no money and make no calls. The canister owns the
records and the escrow, settles and stores, and pays out what an outcome
says. Every rule is testable on the host.

## Install

```toml
[dependencies]
ic-auction = { version = "0.1", features = ["candid", "icrc"] }
```

| Feature  | Adds                                                              |
|----------|-------------------------------------------------------------------|
| (none)   | `harberger` and `vickrey`: pure Rust, `serde` and `sha2` only       |
| `candid` | `CandidType` on the public types, to store or return them          |
| `icrc`   | the `ledger` client; needs `ic-cdk`, so canister code only         |

Minimum Rust: 1.88.

## Example

```rust
use ic_auction::vickrey::{commitment, Auction, Params};

let mut a = Auction::open(Params { commit_ns: 100, reveal_ns: 100, reserve: 10 }, 0);
let (alice, bob) = (b"alice".to_vec(), b"bob".to_vec());
let (sa, sb) = (b"16+ random bytes", b"more random byte");
a.commit(1, alice.clone(), commitment(&alice, 50, sa), 60).unwrap();
a.commit(2, bob.clone(), commitment(&bob, 30, sb), 30).unwrap();
a.reveal(150, &alice, 50, sa).unwrap();
a.reveal(150, &bob, 30, sb).unwrap();
let o = a.outcome(200).unwrap();
assert_eq!((o.winner, o.price), (Some(alice), 30));
```

The crate docs have a Harberger example too, and every type and rule
documented: <https://docs.rs/ic-auction>.

## What the caller must get right

The crate keeps the rules; the canister keeps the money. These are the
places the two meet.

- **Count tax only when you store.** `Harberger::settle` and `top_up`
  return the tax they took. Add it to your treasury only when you write
  the settled record back; a read that settles a copy takes nothing.
  `settled_ns` may trail `now` by under one unit's worth of time: that is
  the rounding carry, not a bug, so store it as returned.
- **Escrow before you commit, pay out after you close.** `Auction::commit`
  assumes the deposit is already held; `Auction::outcome` says what each
  bidder is owed, and paying it is yours. A second commitment from the
  same bidder replaces the first, and `commit` returns the old deposit to
  refund.
- **Salts keep bids sealed.** Amounts are few enough to try one by one
  against a published commitment, so bidders need at least 16 random
  bytes of salt, fresh for every bid, computed on their own machine (not
  by asking a canister, which would send it the bid).
- **Validate auction phases.** A zero-length reveal phase leaves nobody
  time to reveal, and every commitment forfeits. `harberger::Params` has
  `check`; for `vickrey::Params` check that both phases are positive.
- **Undo only what did not move.** `ledger::Failure::nothing_moved` is
  true only for `Rejected` and `Refused`. `FeeCharged` (a cycles-ledger
  withdraw that failed but kept its fee) means undo the amount but not
  the fee. `Undecodable` means the transfer may have happened: undo
  nothing, and record the amount for an operator to reconcile against
  the ledger's blocks, so a retry never pays twice.
- **Pinned fees.** Every transfer pins the fee you pass, so a ledger fee
  change makes the ledger refuse rather than debit more than you budgeted.
  Read it with `ledger::fee` when you configure, and refresh on refusal.

## Used by

[ic-name-service](https://github.com/DrJesseGlass/ic-name-service): flat
names held under the Harberger tax, sold by Vickrey auction when free,
paid for in cycles. Sibling crates:
[ic-multisig](https://github.com/DrJesseGlass/ic-multisig) (K-of-N
approvals), [ic-dev-kit-rs](https://github.com/DrJesseGlass/ic-dev-kit-rs).

## Development

```sh
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
```

The toolchain is pinned in `rust-toolchain.toml`. Source and docs are pure
ASCII. Changes are listed in [CHANGELOG.md](CHANGELOG.md).

```text
src/harberger.rs   Params, Harberger, Status: tax, settle, top_up
src/vickrey.rs     Params, Auction, Bid, Outcome: commit, reveal, outcome
src/ledger.rs      pull, pay, fund_self, fee, Failure (feature icrc)
```

Releases are published from CI. Describe the changes under
`## [Unreleased]` in CHANGELOG.md as you go, then:

```sh
tools/release.sh 0.2.0   # bumps Cargo.toml and Cargo.lock, dates the
                         # CHANGELOG, commits on release-v0.2.0
```

Push that branch, open a PR and merge it, then tag the merge on `main`
with `v<version>` and push the tag. The release workflow checks the tag
matches Cargo.toml and the CHANGELOG has the section, reruns CI,
publishes to crates.io, and creates the GitHub release with that
section as its notes. `v*` tags are protected: once pushed they cannot
be moved or deleted, as a crates.io version cannot be replaced.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this
crate by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.
