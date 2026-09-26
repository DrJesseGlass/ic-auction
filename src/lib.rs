//! Allocation mechanisms for scarce names and slots on the Internet
//! Computer, as pure state machines the canister wraps.
//!
//! - [`harberger`]: a self-assessed price, a prepaid balance, tax settled
//!   lazily on every read and write, a grace period, then free. Anyone may
//!   buy at the assessed price at any time, which is what keeps the price
//!   honest.
//! - [`vickrey`]: a sealed-bid second-price auction with commit and reveal
//!   phases, for allocating something that is free (a first sale, or a
//!   lapsed holding) to the bidder who values it most at the price the
//!   runner-up would have paid.
//! - [`ledger`] (feature `icrc`): the ICRC-1/ICRC-2 client a canister
//!   uses to escrow and pay out in cycles or any ICRC token, with the
//!   failure classification that keeps a retry from paying twice.
//!
//! The mechanisms hold no money and make no calls. The canister owns the
//! records and the escrow: it settles a [`harberger::Harberger`] and stores
//! the result, escrows a deposit when it accepts a [`vickrey`] commitment
//! and pays the [`vickrey::Outcome`] when it closes. Every rule is
//! testable on the host. Amounts are in the caller's unit (cycles, or an
//! ICRC token's smallest unit) and times are nanoseconds, as
//! `ic_cdk::api::time()` returns them.
//!
//! # Harberger tax
//!
//! ```
//! use ic_auction::harberger::{Harberger, Params, Status, YEAR_NS};
//!
//! let p = Params { rate_bps: 700, min_price: 1_000, grace_ns: 1_000_000_000 };
//! let price = 1_000_000_000_000; // 1T cycles
//! p.check_price(price).unwrap();
//! // Take the item with a year of tax prepaid.
//! let mut h = Harberger::new(price, p.tax_per_year(price), 0);
//!
//! // Settle on every read or write; count `taken` only when you store `h`.
//! let (status, taken) = h.settle(&p, (YEAR_NS / 2) as u64);
//! assert_eq!(status, Status::Active);
//! assert_eq!(taken, p.tax_per_year(price) / 2);
//!
//! // A year and a millisecond in, the balance has run out (a holding
//! // lapses once the tax owed exceeds it by a whole unit): grace, then free.
//! let later = YEAR_NS as u64 + 1_000_000;
//! let (status, _) = h.settle(&p, later);
//! assert!(matches!(status, Status::Grace { .. }));
//! assert_eq!(h.status(&p, later + p.grace_ns), Status::Free);
//! ```
//!
//! # Vickrey auction
//!
//! ```
//! use ic_auction::vickrey::{commitment, Auction, Params};
//!
//! let mut a = Auction::open(Params { commit_ns: 100, reveal_ns: 100, reserve: 10 }, 0);
//! let (alice, bob) = (b"alice".to_vec(), b"bob".to_vec());
//! // Salts are random and secret in practice; see `commitment`.
//! let (sa, sb) = (b"16+ random bytes", b"more random byte");
//!
//! // Commit phase: a hash and an escrowed deposit each.
//! a.commit(1, alice.clone(), commitment(&alice, 50, sa), 60).unwrap();
//! a.commit(2, bob.clone(), commitment(&bob, 30, sb), 30).unwrap();
//!
//! // Reveal phase.
//! a.reveal(150, &alice, 50, sa).unwrap();
//! a.reveal(150, &bob, 30, sb).unwrap();
//!
//! // Closed: the highest bid wins at the second-highest.
//! let o = a.outcome(200).unwrap();
//! assert_eq!(o.winner, Some(alice.clone()));
//! assert_eq!(o.price, 30);
//! // Pay these out: alice gets 60 - 30 back, bob all 30.
//! assert_eq!(o.settlements[0].refund, 30);
//! assert_eq!(o.settlements[1].refund, 30);
//! ```
//!
//! # Features
//!
//! - `candid`: derive `CandidType` on the public types, to store them in
//!   stable memory or return them from canister methods.
//! - `icrc`: the [`ledger`] client. Pulls in `ic-cdk` and `candid`, so it
//!   is for use inside a canister.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub mod harberger;
pub mod vickrey;

#[cfg(feature = "icrc")]
#[cfg_attr(docsrs, doc(cfg(feature = "icrc")))]
pub mod ledger;

// The README's examples run as doctests, so they cannot drift.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
