---
name: Bug report
about: Something ic-auction does wrong. Security problems go to the private form instead (SECURITY.md).
title: ''
labels: bug
assignees: ''
---

<!-- If this is a security problem, stop: report it privately at
     https://github.com/DrJesseGlass/ic-auction/security/advisories/new
     Tax that is not charged or is charged twice, an auction won by the
     wrong bidder or at the wrong price, a payout above what was escrowed,
     or a ledger failure reported as "nothing moved" when something did,
     is a security problem. -->

## Version

The `ic-auction` version (`cargo tree -p ic-auction`), the features
enabled (`candid`, `icrc`), the target (host or
`wasm32-unknown-unknown`) and `rustc --version`.

## What you did

The smallest code that shows it: the params, the holding or the auction,
the times, and the call (`settle`, `top_up`, `commit`, `reveal`,
`outcome`, or a `ledger` call and the ledger it went to). A failing test
is ideal.

## What happened

The status, outcome or error you got, or the compiler output. Paste text
as text.

## What you expected

## Anything else

Never paste the salt of a bid in an auction that is still open: it
unseals the bid.
