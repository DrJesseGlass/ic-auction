# Changelog

All notable changes to this crate. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate
follows [Semantic Versioning](https://semver.org/) (while at 0.x, a minor
bump may break the API).

## [Unreleased]

## [0.1.0]

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
