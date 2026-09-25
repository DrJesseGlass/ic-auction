# ic-auction

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
  the canister, read the fee. Fees are pinned on every transfer, and a
  reply the client cannot decode is reported as "may have moved" rather
  than "nothing moved", so a retry never pays twice.

The mechanisms hold no money and make no calls. The canister owns the
records and the escrow, settles and stores, and pays out what an outcome
says. Every rule is testable on the host: `cargo test`.

Features: `candid` derives `CandidType` on the public types; `icrc` adds
the ledger client and needs `ic-cdk`.

Consumer: ic-name-service, where flat names are held under the Harberger
tax and paid for in cycles. Sibling crates: ic-multisig (K-of-N
approvals), ic-dev-kit-rs.

Conventions: pinned toolchain (rust-toolchain.toml), pure ASCII in source
and docs, no timers, no storage.

## Layout

    src/harberger.rs   Params, Harberger, Status: tax, settle, top_up
    src/vickrey.rs     Params, Auction, Bid, Outcome: commit, reveal, outcome
    src/ledger.rs      pull, pay, fund_self, fee, Failure (feature icrc)
