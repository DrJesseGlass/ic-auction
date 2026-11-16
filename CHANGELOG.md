# Changelog

All notable changes to this crate. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate
follows [Semantic Versioning](https://semver.org/) (while at 0.x, a minor
bump may break the API).

## [Unreleased]

### Changed

- The `candid` and `icrc` features now require candid 0.10.40 or later
  (was any 0.10), so taking this release moves a consumer's lock to a
  decoder with the fixes of 0.10.38 to 0.10.40. The old requirement left
  that to each consumer's own `cargo update`. No API change. What the
  fixes mean here:
  - A `vec` longer than the array it is decoded into is refused (0.10.40).
    Earlier decoders kept the first elements and left the rest unread.
    Most such messages then failed with a trailing-value error, but one
    built for it decoded: a 33-byte commitment whose last byte has its
    high bit set, followed by a one-byte deposit of 60, decoded into
    `([u8; 32], u128)` as the first 32 bytes and a deposit of 7685.
    `vickrey::Bid` holds its commitment as `[u8; 32]`, so this reaches a
    canister that decodes a commitment, a `Bid` or an `Auction` from a
    caller. The caller writes the whole message and could have sent the
    decoded values as they are, so by itself it gains them nothing; it
    matters where something else (a wallet, a signer, an indexer, a
    forwarding canister) reads the same bytes and sees different values.
    A canister that takes the commitment as `Vec<u8>` and checks its
    length was not affected. A test now pins the refusal.
  - A `nat` above 128 bits decoded into `u128` is an error at every byte
    of its encoding (0.10.38). This covers the `u128` fields of the
    public types; `ledger` reads fees and block indexes as `Nat` and
    range-checks them itself, so it was not affected.
  - A subtype check no longer accepts a type on the strength of a failed
    probe (0.10.39).

## [0.1.0] - 2026-09-26

First release.

- `harberger`: `Params` (rate, minimum price, grace period) with `check`,
  `tax`, `min_deposit` and `ns_until_spent`; `Harberger` holdings with
  lazy `settle` and `top_up`; `Status` (active, grace, free). Settlement
  carries the rounded-off fraction of tax in time to the next settle, so
  frequent settles cannot round the tax away, and never charges an
  interval twice. `top_up` settles first, refuses a free holding, and
  leaves the holding unchanged when it refuses.
- `vickrey`: sealed-bid second-price `Auction` with commit and reveal
  phases, a reserve, `commitment` hashing bidder, amount and salt under
  a domain tag, and an `Outcome` with per-bidder settlements. Ties go to
  the earlier commitment; unrevealed commitments forfeit.
- `ledger` (feature `icrc`): ICRC-1/ICRC-2 `pull`, `pay`, `fund_self` and
  `fee`, with fees pinned on every transfer and `Failure` telling apart
  nothing moved (`Rejected`, `Refused`), a withdraw that kept its fee
  (`FeeCharged`), and a reply that may have moved funds (`Undecodable`).
- Feature `candid` derives `CandidType` on the public types.

[Unreleased]: https://github.com/DrJesseGlass/ic-auction/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/DrJesseGlass/ic-auction/releases/tag/v0.1.0
