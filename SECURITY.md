# Security

## Reporting a vulnerability

Report it privately, not in a public issue or pull request:

    https://github.com/DrJesseGlass/ic-auction/security/advisories/new

That form opens a draft advisory only the maintainer and you can see. Say
what you found, how to reproduce it (a failing test is the best form), and
which version and features it needs. Expect an acknowledgement within a
week.

A bug that is not a security problem is an ordinary issue:
https://github.com/DrJesseGlass/ic-auction/issues/new/choose

## What counts

ic-auction's promise is that a canister which stores what the crate
returns, and pays what it says, takes exactly the tax that is owed, gives
the thing to the bidder the rules name at the price the rules name, and
never pays out more than it escrowed. Anything that breaks that promise
is in scope, in particular:

- `harberger`: a holding that is charged less tax than its rate, price
  and time owe, beyond the rounding the docs state (`Harberger::settle`);
  an interval charged twice; a `top_up` that forgives tax owed, or brings
  a holding back from grace for less than `Params::min_deposit`; a free
  holding that becomes active again; a `Status` that says `Active` or
  `Grace` for a holding whose balance and grace period have both run out;
- `vickrey`: a `reveal` accepted for an amount or salt other than the
  ones committed, or for a bidder other than the one who committed; two
  different (bidder, amount, salt) that hash to the same bytes
  (`commitment`); a `commit` or `reveal` accepted outside its phase; a
  revealed bid above its deposit; an `Outcome` whose winner is not the
  highest revealed bid at or above the reserve, or whose price is not the
  second-highest or the reserve; settlements that refund an unrevealed
  commitment, or that together with the price exceed the deposits held;
- `ledger` (`icrc` feature): a `Failure` for which `nothing_moved` is
  true when units moved or may have; a transfer sent without the fee the
  caller pinned; a ledger reply decoded to a different amount, fee or
  block index than the ledger sent; a `pull` that debits or credits an
  account other than the payer's and this canister's;
- decoding and arithmetic: a serde or candid encoding that decodes to a
  different record than was encoded, or a panic or overflow in any of the
  above on input a caller cannot rule out (bidder bytes, salts, amounts,
  times), under parameters `harberger::Params::check` and `check_price`
  accept.

## What does not

- The crate holds no money and makes no calls outside `ledger`. What the
  README lists under "What the caller must get right" is the caller's:
  tax counted on a read that was not stored, a commitment accepted before
  its deposit was escrowed, an outcome not paid, a transfer retried after
  `Failure::Undecodable` and paid twice.
- A short, reused or guessable salt lets anyone try amounts against a
  published commitment. That is the documented limit of a hash commitment
  (`vickrey::commitment`): 16 random bytes, fresh for every bid, made on
  the bidder's own machine.
- `vickrey::Params` has no `check`. A zero-length phase, where nobody can
  commit or every commitment forfeits, is the caller's to refuse (README).
- Deposits are passed to `commit` in the clear and bound the bid from
  above, and a bidder may choose not to reveal and forfeit the deposit,
  which can change the price the winner pays. Those, and shill bids and
  collusion among bidders, are properties of the mechanism, not of this
  code.
- A Harberger holder who assesses a low price and is bought out at it,
  and the under-one-unit rounding per settle that `Harberger::settle`
  documents (`settled_ns` trailing `now` is the carry, not lost tax).
- Every time is the `now` the caller passes. A canister that passes
  anything but `ic_cdk::api::time()` has chosen its own clock.
- `ledger::fund_self` cannot pin a fee, because `withdraw` has no fee
  field; the caller checks the live fee first, as its documentation says.
  A ledger that itself misbehaves is the ledger's to fix.
- A dependency advisory with no reachable path in this code. Dependabot
  files those (`.github/dependabot.yml`); if you believe a path exists,
  that is a report.
- A vulnerability in a canister that uses this crate, unless the crate is
  the cause.

## Supported versions

Only the latest release on crates.io. A fix is a new version; older ones
are not patched.

## How a fix ships

A fix is a release to crates.io by the tag-driven workflow
(`.github/workflows/release.yml`), with its CHANGELOG entry, and with the
affected versions yanked where the flaw warrants it. The canisters that
consume the crate take it by a version bump, which changes their module
hashes, so each ships it by its own release path. The advisory is
published, with credit to the reporter if wanted, once the fixed version
is on crates.io and the known consumers have had the chance to take it.
