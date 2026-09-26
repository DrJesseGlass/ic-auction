//! Harberger tax with lazy settlement.
//!
//! The holder of a scarce thing self-assesses a price P and prepays a
//! balance. Tax accrues on P at `rate_bps` per year and is settled lazily:
//! on every read or write, the tax due since `settled_ns` comes off the
//! balance. When the balance runs out the holder keeps the thing for a
//! grace period, after which it is free to take. Anyone may buy it at P at
//! any time; the seller gets P plus the unspent balance.
//!
//! No timers, no per-item bookkeeping beyond three numbers, and no storage
//! or calls: the canister owns the record and the money, this module is
//! arithmetic and rules. Every amount is in the caller's unit (cycles, or
//! an ICRC token's smallest unit); every time is nanoseconds.

use serde::{Deserialize, Serialize};

pub const YEAR_NS: u128 = 365 * 24 * 60 * 60 * 1_000_000_000;

/// Prices above this are refused: 10^18 units, which keeps the tax over
/// any u64 interval inside u128 with room to spare.
pub const MAX_PRICE: u128 = 1_000_000_000_000_000_000;

/// The rules a Harberger market runs under.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Params {
    /// Tax per year on the assessed price, in basis points (700 = 7%).
    pub rate_bps: u32,
    /// Lowest price anyone may assess.
    pub min_price: u128,
    /// How long a holder with no balance keeps the thing.
    pub grace_ns: u64,
}

impl Params {
    /// Refuse rates above 100% a year, a minimum above the arithmetic cap,
    /// and a zero grace period (every item would be free at once).
    pub fn check(&self) -> Result<(), String> {
        if self.rate_bps > 10_000 {
            return Err("rate_bps above 10000 (100% per year)".to_string());
        }
        if self.min_price > MAX_PRICE {
            return Err(format!("min_price above the maximum price of {MAX_PRICE}"));
        }
        if self.grace_ns == 0 {
            return Err("grace_ns is zero: every item would be free at once".to_string());
        }
        Ok(())
    }

    pub fn check_price(&self, price: u128) -> Result<(), String> {
        if price < self.min_price {
            return Err(format!("price below the minimum of {}", self.min_price));
        }
        if price > MAX_PRICE {
            return Err(format!("price above the maximum of {MAX_PRICE}"));
        }
        Ok(())
    }

    /// Tax per year on `price`.
    pub fn tax_per_year(&self, price: u128) -> u128 {
        price.saturating_mul(self.rate_bps as u128) / 10_000
    }

    /// Tax on `price` over `elapsed_ns`. Computed as the yearly tax first
    /// so the product stays small; saturates rather than overflowing for
    /// prices outside MAX_PRICE.
    pub fn tax(&self, price: u128, elapsed_ns: u64) -> u128 {
        self.tax_per_year(price)
            .checked_mul(elapsed_ns as u128)
            .map(|x| x / YEAR_NS)
            .unwrap_or(u128::MAX)
    }

    /// Nanoseconds until `balance` is spent on the tax on `price`, or None
    /// when the tax is zero (never).
    pub fn ns_until_spent(&self, price: u128, balance: u128) -> Option<u64> {
        let per_year = self.tax_per_year(price);
        if per_year == 0 {
            return None;
        }
        let ns = balance.checked_mul(YEAR_NS)? / per_year;
        Some(ns.min(u64::MAX as u128) as u64)
    }

    /// The smallest deposit to accept when taking or buying: the tax for
    /// one grace period, so nothing is ever held on credit.
    pub fn min_deposit(&self, price: u128) -> u128 {
        self.tax(price, self.grace_ns)
    }

    pub fn check_deposit(&self, price: u128, deposit: u128) -> Result<(), String> {
        let min = self.min_deposit(price);
        if deposit < min {
            return Err(format!(
                "deposit must cover one grace period of tax: at least {min} at this price"
            ));
        }
        Ok(())
    }
}

/// The three numbers per held item, plus when the balance ran out.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Harberger {
    /// Self-assessed price. Anyone may buy for this.
    pub price: u128,
    /// Prepaid tax as of `settled_ns`.
    pub balance: u128,
    pub settled_ns: u64,
    /// When the balance ran out, if it has; the grace period runs from here.
    pub lapsed_ns: Option<u64>,
}

impl Harberger {
    /// A fresh holding: `deposit` prepaid at `price`, settled to `now`.
    pub fn new(price: u128, deposit: u128, now: u64) -> Self {
        Harberger {
            price,
            balance: deposit,
            settled_ns: now,
            lapsed_ns: None,
        }
    }

    /// Settle tax up to `now` in place. Returns the status and the tax
    /// taken, which the caller counts only when it stores the result.
    pub fn settle(&mut self, p: &Params, now: u64) -> (Status, u128) {
        let mut taken = 0u128;
        if self.lapsed_ns.is_none() && now > self.settled_ns {
            let elapsed = now - self.settled_ns;
            let due = p.tax(self.price, elapsed);
            if due <= self.balance {
                self.balance -= due;
                taken = due;
                // Advance over the least time whose tax is `due`, so the
                // fraction the division floored away is carried to the next
                // settle instead of forgiven (settling often enough, every
                // read, would otherwise round each `due` to zero). Rounding
                // up matters above one unit of tax per nanosecond: rounded
                // down, a charged `due` could advance no time at all and a
                // second settle at the same `now` would charge it again.
                // The least such time is never more than `elapsed`, and what
                // is left of the interval owes less than one unit, so a
                // second settle at `now` takes nothing. The carry lives in
                // whole nanoseconds, so above one unit per ns each settle
                // still forgives under one unit, as plain flooring would.
                let per_year = p.tax_per_year(self.price);
                let paid_ns = if per_year == 0 {
                    elapsed
                } else {
                    due.checked_mul(YEAR_NS)
                        .and_then(|x| x.checked_add(per_year - 1))
                        .map(|x| (x / per_year).min(elapsed as u128) as u64)
                        .unwrap_or(elapsed)
                };
                self.settled_ns += paid_ns;
            } else {
                // The balance ran out somewhere in the interval; find when.
                let lapsed_at = match p.ns_until_spent(self.price, self.balance) {
                    Some(ns) => self.settled_ns.saturating_add(ns).min(now),
                    None => now,
                };
                taken = self.balance;
                self.balance = 0;
                self.lapsed_ns = Some(lapsed_at);
                self.settled_ns = now;
            }
        }
        (self.status(p, now), taken)
    }

    /// Status as of `now` without settling (the balance may be stale).
    pub fn status(&self, p: &Params, now: u64) -> Status {
        match self.lapsed_ns {
            None => Status::Active,
            Some(t) if now < t.saturating_add(p.grace_ns) => Status::Grace {
                until_ns: t.saturating_add(p.grace_ns),
            },
            Some(_) => Status::Free,
        }
    }

    /// Add prepaid tax. A holding in grace comes back to active only if
    /// the balance afterwards covers one grace period of tax, as a fresh
    /// take must; otherwise dust would buy a fresh grace period each time.
    /// Settles to `now` first, so tax owed is never forgiven by a top-up,
    /// and returns the tax that settle took (counted, like [`Self::settle`]'s,
    /// only when the caller stores the result). A free holding cannot be
    /// topped up: it is anyone's to take. On Err the holding is unchanged,
    /// settlement included, so a refused top-up leaves nothing to store.
    pub fn top_up(&mut self, p: &Params, amount: u128, now: u64) -> Result<u128, String> {
        let mut next = self.clone();
        let (status, taken) = next.settle(p, now);
        if status == Status::Free {
            return Err("the grace period is over: the item is free".to_string());
        }
        let after = next.balance.saturating_add(amount);
        let min = p.min_deposit(next.price);
        if after < min {
            return Err(format!(
                "balance after the deposit must cover one grace period of tax: at least {min} at this price, {} left",
                next.balance
            ));
        }
        next.balance = after;
        if next.lapsed_ns.take().is_some() {
            next.settled_ns = now;
        }
        *self = next;
        Ok(taken)
    }
}

#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Balance covers the tax so far.
    #[serde(rename = "active")]
    Active,
    /// Balance ran out; the holder keeps the item until `until_ns`.
    #[serde(rename = "grace")]
    Grace { until_ns: u64 },
    /// Grace over: anyone may take the item.
    #[serde(rename = "free")]
    Free,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            rate_bps: 700,
            min_price: 100_000_000_000,
            grace_ns: 10 * 1_000_000_000,
        }
    }

    #[test]
    fn tax_math() {
        let p = params();
        assert_eq!(p.tax(1_000_000_000_000, YEAR_NS as u64), 70_000_000_000);
        assert_eq!(p.tax_per_year(1_000_000_000_000), 70_000_000_000);
        assert_eq!(p.tax(1_000_000_000_000, 0), 0);
        assert!(p.tax(MAX_PRICE, u64::MAX) < u128::MAX);
        assert_eq!(p.tax(u128::MAX, u64::MAX), u128::MAX);
        assert_eq!(
            p.min_deposit(1_000_000_000_000),
            p.tax(1_000_000_000_000, p.grace_ns)
        );
    }

    #[test]
    fn params_checks() {
        assert!(params().check().is_ok());
        assert!(Params {
            rate_bps: 20_000,
            ..params()
        }
        .check()
        .is_err());
        assert!(Params {
            grace_ns: 0,
            ..params()
        }
        .check()
        .is_err());
        assert!(Params {
            min_price: MAX_PRICE + 1,
            ..params()
        }
        .check()
        .is_err());
        assert!(params().check_price(1).is_err());
        assert!(params().check_price(MAX_PRICE + 1).is_err());
        assert!(params().check_price(1_000_000_000_000).is_ok());
        assert!(params().check_deposit(1_000_000_000_000, 1).is_err());
    }

    #[test]
    fn settles_lapses_and_frees() {
        let p = params();
        let price = 1_000_000_000_000u128;
        let per_year = p.tax_per_year(price);
        let mut h = Harberger::new(price, per_year, 0);
        let half = YEAR_NS as u64 / 2;
        let (s, taken) = h.settle(&p, half);
        assert_eq!(s, Status::Active);
        assert_eq!(taken, per_year / 2);
        assert_eq!(h.balance, per_year - per_year / 2);
        assert_eq!(h.settled_ns, half);
        let year = YEAR_NS as u64;
        let (s, taken) = h.settle(&p, year + 5_000_000_000);
        assert_eq!(
            s,
            Status::Grace {
                until_ns: h.lapsed_ns.unwrap() + p.grace_ns
            }
        );
        assert_eq!(taken, per_year - per_year / 2);
        assert_eq!(h.balance, 0);
        let lapsed = h.lapsed_ns.unwrap();
        assert!(
            lapsed >= year - 1_000 && lapsed <= year + 1_000,
            "lapsed {lapsed} vs {year}"
        );
        let (s, taken) = h.settle(&p, lapsed + 5_000_000_000);
        assert!(matches!(s, Status::Grace { .. }));
        assert_eq!(taken, 0);
        let (s, _) = h.settle(&p, lapsed + p.grace_ns + 1);
        assert_eq!(s, Status::Free);
    }

    #[test]
    fn top_up_rules() {
        let p = params();
        let price = 1_000_000_000_000u128;
        let mut h = Harberger::new(price, 0, 0);
        h.lapsed_ns = Some(0);
        assert!(h.top_up(&p, 1, 5).is_err());
        assert!(h.top_up(&p, p.min_deposit(price), 5).is_ok());
        assert_eq!(h.lapsed_ns, None);
        assert_eq!(h.settled_ns, 5);
        assert_eq!(h.status(&p, 6), Status::Active);
    }

    #[test]
    fn frequent_settles_do_not_round_the_tax_away() {
        let p = params();
        let price = 100_000_000u128;
        let per_year = p.tax_per_year(price);
        let mut h = Harberger::new(price, per_year, 0);
        // At this price one second of tax is under one unit.
        let second = 1_000_000_000u64;
        assert_eq!(p.tax(price, second), 0);
        let mut total = 0;
        let hour = 3_600u64;
        for i in 1..=hour {
            total += h.settle(&p, i * second).1;
        }
        assert_eq!(total, p.tax(price, hour * second));
    }

    #[test]
    fn top_up_settles_first_and_refuses_free() {
        let p = params();
        let price = 1_000_000_000_000u128;
        let per_year = p.tax_per_year(price);
        let mut h = Harberger::new(price, per_year, 0);
        let half = YEAR_NS as u64 / 2;
        assert_eq!(h.top_up(&p, 1, half), Ok(per_year / 2));
        assert_eq!(h.balance, per_year - per_year / 2 + 1);
        let mut free = Harberger::new(price, 0, 0);
        free.lapsed_ns = Some(0);
        let before = free.clone();
        assert!(free.top_up(&p, per_year, p.grace_ns + 1).is_err());
        assert_eq!(free, before);
    }

    #[test]
    fn a_refused_top_up_changes_nothing() {
        let p = params();
        let price = 1_000_000_000_000u128;
        let per_year = p.tax_per_year(price);
        // Half a year of tax owed, and a top-up too small to reach one
        // grace period of tax afterwards.
        let mut h = Harberger::new(price, per_year / 2 + 1, 0);
        let before = h.clone();
        let half = YEAR_NS as u64 / 2;
        assert!(h.top_up(&p, 0, half).is_err());
        assert_eq!(h, before);
    }

    #[test]
    fn a_high_rate_settle_charges_an_instant_once() {
        // 100% a year at the maximum price is about 31.7 units per ns, so
        // a one-ns interval charges 31 units.
        let p = Params {
            rate_bps: 10_000,
            ..params()
        };
        let mut h = Harberger::new(MAX_PRICE, MAX_PRICE, 0);
        let (_, first) = h.settle(&p, 1);
        assert_eq!(first, p.tax(MAX_PRICE, 1));
        assert!(first > 0);
        assert_eq!(h.settle(&p, 1).1, 0);
        assert_eq!(h.settle(&p, 1).1, 0);
        // Settling every ns: no interval is charged twice, and each settle
        // forgives less than one unit (the carry lives in whole ns, and
        // here one ns is worth more than a unit).
        let mut total = first;
        let n = 1_000u64;
        for now in 2..=n {
            total += h.settle(&p, now).1;
            assert_eq!(h.settle(&p, now).1, 0);
        }
        let exact = p.tax(MAX_PRICE, n);
        assert!(total <= exact && total + n as u128 > exact);
    }
}
