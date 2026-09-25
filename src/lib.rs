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
//! testable on the host. Under the `candid` feature the public types derive
//! `CandidType`.
//!
//! Consumer: ic-name-service (flat names under a Harberger tax; Vickrey
//! for names coming to market). Written so the next consumer needs no
//! change here.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod harberger;
pub mod vickrey;

#[cfg(feature = "icrc")]
#[cfg_attr(docsrs, doc(cfg(feature = "icrc")))]
pub mod ledger;
