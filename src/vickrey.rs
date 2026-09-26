//! Sealed-bid second-price (Vickrey) auction, as a pure state machine.
//!
//! Bidders commit `sha256(domain || bidder || amount || salt)` with a
//! deposit during the commit phase, reveal amount and salt during the
//! reveal phase, and after that the highest revealed bid wins and pays the
//! second-highest revealed bid, or the reserve if there is only one. A
//! bidder who never reveals forfeits the deposit (so a commitment is a
//! promise, not a free option); a revealed bid above its deposit is
//! refused at reveal time.
//!
//! The module holds no money and makes no calls. The canister escrows the
//! deposits when it accepts a commitment and pays out the [`Outcome`]'s
//! settlements when it closes the auction. Bidders are opaque bytes (a
//! principal's bytes, say) so nothing here depends on the IC.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"ic-auction/vickrey/v1";

/// A bidder's identity as opaque bytes (a principal's bytes, say).
pub type Bidder = Vec<u8>;

/// Phase lengths and reserve, fixed when an auction opens. Both phases
/// should be positive: a zero reveal phase leaves no time to reveal, and
/// every commitment would forfeit.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Params {
    /// Length of the commit phase from opening.
    pub commit_ns: u64,
    /// Length of the reveal phase after the commit phase.
    pub reveal_ns: u64,
    /// The winner pays at least this; a lone bid pays exactly this.
    pub reserve: u128,
}

/// Where an auction stands at a given time.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Commitments accepted.
    #[serde(rename = "commit")]
    Commit,
    /// Commitments opened; no new ones.
    #[serde(rename = "reveal")]
    Reveal,
    /// Over: [`Auction::outcome`] is available.
    #[serde(rename = "closed")]
    Closed,
}

/// One bidder's commitment and, once opened, their amount.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Bid {
    /// Who committed.
    pub bidder: Bidder,
    /// [`commitment`] of bidder, amount and salt.
    pub commitment: [u8; 32],
    /// Escrowed by the canister; a revealed amount may not exceed it.
    pub deposit: u128,
    /// When the commitment was made; ties go to the earlier one.
    pub committed_ns: u64,
    /// The amount, once revealed.
    pub revealed: Option<u128>,
}

/// A sealed-bid second-price auction: the whole state, to store as is.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Auction {
    /// Fixed at opening.
    pub params: Params,
    /// When the commit phase started.
    pub opened_ns: u64,
    /// One per bidder; a second commitment from the same bidder replaces
    /// the first (the canister refunds the first deposit).
    pub bids: Vec<Bid>,
}

/// What each bidder is owed when the auction closes.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Settlement {
    /// Whose deposit this settles.
    pub bidder: Bidder,
    /// Deposit returned (for the winner, deposit minus the price).
    pub refund: u128,
    /// Deposit kept: an unrevealed commitment.
    pub forfeited: u128,
}

/// The result of a closed auction: who won, what they pay, and what every
/// bidder is owed.
#[cfg_attr(feature = "candid", derive(candid::CandidType))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// None when nobody revealed a bid at or above the reserve.
    pub winner: Option<Bidder>,
    /// What the winner pays: the second-highest revealed bid, or the
    /// reserve when there is no second bid or it is below the reserve.
    pub price: u128,
    /// One per commitment, in commitment order.
    pub settlements: Vec<Settlement>,
}

/// The commitment a bidder publishes: binds bidder, amount and salt so a
/// reveal cannot be replayed by or for anyone else. The salt is what
/// keeps the amount sealed: amounts are few enough to try one by one, so
/// use at least 16 random bytes, fresh for every bid.
pub fn commitment(bidder: &[u8], amount: u128, salt: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update((bidder.len() as u64).to_be_bytes());
    h.update(bidder);
    h.update(amount.to_be_bytes());
    h.update((salt.len() as u64).to_be_bytes());
    h.update(salt);
    h.finalize().into()
}

impl Auction {
    /// Open an auction whose commit phase starts at `now`.
    pub fn open(params: Params, now: u64) -> Self {
        Auction {
            params,
            opened_ns: now,
            bids: Vec::new(),
        }
    }

    /// When the commit phase ends and the reveal phase starts.
    pub fn commit_until_ns(&self) -> u64 {
        self.opened_ns.saturating_add(self.params.commit_ns)
    }

    /// When the reveal phase ends and the auction is closed.
    pub fn reveal_until_ns(&self) -> u64 {
        self.commit_until_ns().saturating_add(self.params.reveal_ns)
    }

    /// The phase at `now`.
    pub fn phase(&self, now: u64) -> Phase {
        if now < self.commit_until_ns() {
            Phase::Commit
        } else if now < self.reveal_until_ns() {
            Phase::Reveal
        } else {
            Phase::Closed
        }
    }

    /// Record a commitment with its escrowed deposit. Returns the deposit
    /// of a replaced earlier commitment by the same bidder, for the
    /// canister to refund.
    pub fn commit(
        &mut self,
        now: u64,
        bidder: Bidder,
        commitment: [u8; 32],
        deposit: u128,
    ) -> Result<Option<u128>, String> {
        if self.phase(now) != Phase::Commit {
            return Err("the commit phase is over".to_string());
        }
        if deposit < self.params.reserve {
            return Err(format!(
                "deposit must be at least the reserve of {}",
                self.params.reserve
            ));
        }
        let replaced = match self.bids.iter().position(|b| b.bidder == bidder) {
            Some(i) => Some(self.bids.remove(i).deposit),
            None => None,
        };
        self.bids.push(Bid {
            bidder,
            commitment,
            deposit,
            committed_ns: now,
            revealed: None,
        });
        Ok(replaced)
    }

    /// Open a commitment. The amount must be covered by the deposit.
    pub fn reveal(
        &mut self,
        now: u64,
        bidder: &[u8],
        amount: u128,
        salt: &[u8],
    ) -> Result<(), String> {
        match self.phase(now) {
            Phase::Reveal => {}
            Phase::Commit => return Err("the reveal phase has not started".to_string()),
            Phase::Closed => return Err("the reveal phase is over".to_string()),
        }
        let bid = self
            .bids
            .iter_mut()
            .find(|b| b.bidder == bidder)
            .ok_or_else(|| "no commitment from this bidder".to_string())?;
        if bid.revealed.is_some() {
            return Err("already revealed".to_string());
        }
        if commitment(bidder, amount, salt) != bid.commitment {
            return Err("amount and salt do not match the commitment".to_string());
        }
        if amount > bid.deposit {
            return Err(format!(
                "revealed amount {amount} exceeds the deposit of {}",
                bid.deposit
            ));
        }
        bid.revealed = Some(amount);
        Ok(())
    }

    /// The result, once the reveal phase is over. Ties go to the earlier
    /// commitment.
    pub fn outcome(&self, now: u64) -> Result<Outcome, String> {
        if self.phase(now) != Phase::Closed {
            return Err("the auction is still open".to_string());
        }
        let reserve = self.params.reserve;
        let mut ranked: Vec<&Bid> = self
            .bids
            .iter()
            .filter(|b| b.revealed.is_some_and(|a| a >= reserve))
            .collect();
        ranked.sort_by(|a, b| {
            b.revealed
                .cmp(&a.revealed)
                .then(a.committed_ns.cmp(&b.committed_ns))
        });
        let winner = ranked.first().map(|b| b.bidder.clone());
        // Everything ranked revealed at or above the reserve.
        let price = match ranked.first() {
            Some(_) => ranked.get(1).and_then(|b| b.revealed).unwrap_or(reserve),
            None => 0,
        };
        let settlements = self
            .bids
            .iter()
            .map(|b| {
                let is_winner = winner.as_ref() == Some(&b.bidder);
                match b.revealed {
                    None => Settlement {
                        bidder: b.bidder.clone(),
                        refund: 0,
                        forfeited: b.deposit,
                    },
                    Some(_) if is_winner => Settlement {
                        bidder: b.bidder.clone(),
                        refund: b.deposit.saturating_sub(price),
                        forfeited: 0,
                    },
                    Some(_) => Settlement {
                        bidder: b.bidder.clone(),
                        refund: b.deposit,
                        forfeited: 0,
                    },
                }
            })
            .collect();
        Ok(Outcome {
            winner,
            price,
            settlements,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            commit_ns: 100,
            reveal_ns: 100,
            reserve: 10,
        }
    }

    fn bidder(n: u8) -> Bidder {
        vec![n; 4]
    }

    #[test]
    fn phases() {
        let a = Auction::open(params(), 1000);
        assert_eq!(a.phase(1000), Phase::Commit);
        assert_eq!(a.phase(1099), Phase::Commit);
        assert_eq!(a.phase(1100), Phase::Reveal);
        assert_eq!(a.phase(1199), Phase::Reveal);
        assert_eq!(a.phase(1200), Phase::Closed);
    }

    #[test]
    fn second_price_with_refunds_and_forfeits() {
        let mut a = Auction::open(params(), 0);
        let (s1, s2, s3) = (b"salt1", b"salt2", b"salt3");
        a.commit(1, bidder(1), commitment(&bidder(1), 50, s1), 60)
            .unwrap();
        a.commit(2, bidder(2), commitment(&bidder(2), 80, s2), 80)
            .unwrap();
        a.commit(3, bidder(3), commitment(&bidder(3), 30, s3), 30)
            .unwrap();
        // Too early to reveal, too late to commit.
        assert!(a.reveal(50, &bidder(1), 50, s1).is_err());
        assert!(a.commit(150, bidder(4), [0; 32], 20).is_err());
        a.reveal(150, &bidder(1), 50, s1).unwrap();
        a.reveal(150, &bidder(2), 80, s2).unwrap();
        // Bidder 3 never reveals and forfeits.
        assert!(a.outcome(150).is_err());
        let o = a.outcome(200).unwrap();
        assert_eq!(o.winner, Some(bidder(2)));
        assert_eq!(o.price, 50);
        let by = |n: u8| {
            o.settlements
                .iter()
                .find(|s| s.bidder == bidder(n))
                .unwrap()
                .clone()
        };
        assert_eq!(
            by(1),
            Settlement {
                bidder: bidder(1),
                refund: 60,
                forfeited: 0
            }
        );
        assert_eq!(
            by(2),
            Settlement {
                bidder: bidder(2),
                refund: 30,
                forfeited: 0
            }
        );
        assert_eq!(
            by(3),
            Settlement {
                bidder: bidder(3),
                refund: 0,
                forfeited: 30
            }
        );
    }

    #[test]
    fn lone_bid_pays_reserve_and_below_reserve_loses() {
        let mut a = Auction::open(params(), 0);
        a.commit(1, bidder(1), commitment(&bidder(1), 70, b"s"), 70)
            .unwrap();
        a.commit(2, bidder(2), commitment(&bidder(2), 5, b"t"), 10)
            .unwrap();
        a.reveal(150, &bidder(1), 70, b"s").unwrap();
        a.reveal(150, &bidder(2), 5, b"t").unwrap();
        let o = a.outcome(200).unwrap();
        assert_eq!(o.winner, Some(bidder(1)));
        assert_eq!(o.price, 10);
        assert_eq!(o.settlements[0].refund, 60);
        assert_eq!(o.settlements[1].refund, 10);
    }

    #[test]
    fn nobody_wins_when_nothing_reveals_at_reserve() {
        let mut a = Auction::open(params(), 0);
        a.commit(1, bidder(1), commitment(&bidder(1), 5, b"s"), 10)
            .unwrap();
        a.reveal(150, &bidder(1), 5, b"s").unwrap();
        let o = a.outcome(200).unwrap();
        assert_eq!(o.winner, None);
        assert_eq!(o.price, 0);
        assert_eq!(o.settlements[0].refund, 10);
    }

    #[test]
    fn reveal_checks() {
        let mut a = Auction::open(params(), 0);
        a.commit(1, bidder(1), commitment(&bidder(1), 50, b"s"), 40)
            .unwrap();
        assert!(a.reveal(150, &bidder(1), 50, b"wrong").is_err());
        assert!(a.reveal(150, &bidder(2), 50, b"s").is_err());
        // Bid above the deposit is refused; the commitment stays unrevealed.
        assert!(a.reveal(150, &bidder(1), 50, b"s").is_err());
        assert_eq!(a.outcome(200).unwrap().settlements[0].forfeited, 40);
    }

    #[test]
    fn recommit_replaces_and_reports_the_old_deposit() {
        let mut a = Auction::open(params(), 0);
        assert_eq!(a.commit(1, bidder(1), [1; 32], 20).unwrap(), None);
        assert_eq!(a.commit(2, bidder(1), [2; 32], 30).unwrap(), Some(20));
        assert_eq!(a.bids.len(), 1);
        assert!(a.commit(3, bidder(2), [3; 32], 5).is_err());
    }

    #[test]
    fn ties_go_to_the_earlier_commitment() {
        let mut a = Auction::open(params(), 0);
        a.commit(2, bidder(2), commitment(&bidder(2), 50, b"b"), 50)
            .unwrap();
        a.commit(1, bidder(1), commitment(&bidder(1), 50, b"a"), 50)
            .unwrap();
        a.reveal(150, &bidder(1), 50, b"a").unwrap();
        a.reveal(150, &bidder(2), 50, b"b").unwrap();
        let o = a.outcome(200).unwrap();
        assert_eq!(o.winner, Some(bidder(1)));
        assert_eq!(o.price, 50);
    }

    #[test]
    fn commitment_binds_bidder_amount_and_salt() {
        let c = commitment(&bidder(1), 50, b"s");
        assert_ne!(c, commitment(&bidder(2), 50, b"s"));
        assert_ne!(c, commitment(&bidder(1), 51, b"s"));
        assert_ne!(c, commitment(&bidder(1), 50, b"t"));
        assert_eq!(c, commitment(&bidder(1), 50, b"s"));
    }
}
