//! The cycles ledger (or any ICRC-1/ICRC-2 ledger), from inside a canister.
//!
//! Four calls. [`pull`] moves units from a payer's account into this
//! canister's account under the payer's ICRC-2 approval. [`pay`] moves
//! units from this canister's account to a principal. [`fund_self`] turns
//! ledger cycles into this canister's own cycles balance (cycles ledger
//! only). [`fee`] reads the transfer fee. Every transfer pins the fee it
//! was budgeted for, so a fee change makes the ledger refuse rather than
//! debit more than the caller's books record.
//!
//! [`Failure`] tells apart a rejected call and a refused transfer, where
//! nothing moved, from a reply this code cannot decode, where the
//! transfer may have happened. A caller undoes its own bookkeeping only
//! in the first two cases.
//!
//! Enabled by the `icrc` feature; needs ic-cdk.

use candid::{CandidType, Nat, Principal};
use ic_cdk::call::Call;
use serde::Deserialize;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Vec<u8>>,
}

fn account(owner: Principal) -> Account {
    Account {
        owner,
        subaccount: None,
    }
}

#[derive(CandidType, Deserialize, Debug)]
struct TransferFromArgs {
    to: Account,
    fee: Option<Nat>,
    spender_subaccount: Option<Vec<u8>>,
    from: Account,
    memo: Option<Vec<u8>>,
    created_at_time: Option<u64>,
    amount: Nat,
}

#[derive(CandidType, Deserialize, Debug)]
struct TransferArgs {
    to: Account,
    fee: Option<Nat>,
    memo: Option<Vec<u8>>,
    from_subaccount: Option<Vec<u8>>,
    created_at_time: Option<u64>,
    amount: Nat,
}

#[derive(CandidType, Deserialize, Debug)]
struct WithdrawArgs {
    to: Principal,
    from_subaccount: Option<Vec<u8>>,
    created_at_time: Option<u64>,
    amount: Nat,
}

/// The union of the error variants of icrc1_transfer, icrc2_transfer_from
/// and withdraw. Candid lets a decoder declare more variants than a value
/// carries, so one enum serves all three methods.
#[derive(CandidType, Deserialize, Debug)]
enum LedgerError {
    GenericError {
        message: String,
        error_code: Nat,
    },
    TemporarilyUnavailable,
    BadBurn {
        min_burn_amount: Nat,
    },
    Duplicate {
        duplicate_of: Nat,
    },
    BadFee {
        expected_fee: Nat,
    },
    CreatedInFuture {
        ledger_time: u64,
    },
    TooOld,
    InsufficientFunds {
        balance: Nat,
    },
    InsufficientAllowance {
        allowance: Nat,
    },
    FailedToWithdraw {
        rejection_code: RejectionCode,
        fee_block: Option<Nat>,
        rejection_reason: String,
    },
    InvalidReceiver {
        receiver: Principal,
    },
}

#[derive(CandidType, Deserialize, Debug)]
enum RejectionCode {
    NoError,
    SysFatal,
    SysTransient,
    DestinationInvalid,
    CanisterReject,
    CanisterError,
    Unknown,
}

#[derive(CandidType, Deserialize, Debug)]
enum LedgerResult {
    Ok(Nat),
    Err(LedgerError),
}

/// How a ledger call failed, and whether the cycles moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The call was rejected: the ledger did not run it, or trapped. No
    /// transfer happened, so the caller's own bookkeeping can be undone.
    Rejected(String),
    /// The ledger ran and answered Err. No transfer happened.
    Refused(String),
    /// The ledger ran and answered something this code cannot decode. The
    /// transfer may well have happened. The caller must not undo its
    /// bookkeeping; it records the amount as unreconciled instead.
    Undecodable(String),
}

impl Failure {
    /// True when it is certain no cycles moved.
    pub fn nothing_moved(&self) -> bool {
        !matches!(self, Failure::Undecodable(_))
    }

    pub fn message(&self) -> String {
        match self {
            Failure::Rejected(m) | Failure::Refused(m) => m.clone(),
            Failure::Undecodable(m) => format!(
                "{m}; the transfer may have gone through, so nothing was undone and the amount is recorded as unreconciled for the operator"
            ),
        }
    }
}

async fn call(ledger: Principal, method: &str, arg: impl CandidType) -> Result<u128, Failure> {
    let res = Call::unbounded_wait(ledger, method)
        .with_arg(arg)
        .await
        .map_err(|e| Failure::Rejected(format!("{method}: {e:?}")))?;
    match res.candid::<LedgerResult>() {
        Ok(LedgerResult::Ok(n)) => Ok(nat_to_u128(n)),
        Ok(LedgerResult::Err(e)) => {
            Err(Failure::Refused(format!("{method}: ledger refused: {e:?}")))
        }
        Err(e) => Err(Failure::Undecodable(format!(
            "{method}: undecodable reply: {e}"
        ))),
    }
}

fn nat_to_u128(n: Nat) -> u128 {
    let bytes = n.0.to_bytes_le();
    let mut buf = [0u8; 16];
    let len = bytes.len().min(16);
    buf[..len].copy_from_slice(&bytes[..len]);
    u128::from_le_bytes(buf)
}

/// The ledger's current transfer fee.
pub async fn fee(ledger: Principal) -> Result<u128, Failure> {
    let res = Call::unbounded_wait(ledger, "icrc1_fee")
        .await
        .map_err(|e| Failure::Rejected(format!("icrc1_fee: {e:?}")))?;
    res.candid::<Nat>()
        .map(nat_to_u128)
        .map_err(|e| Failure::Undecodable(format!("icrc1_fee: undecodable reply: {e}")))
}

/// Pull `amount` cycles from `payer` into this canister's ledger account.
/// The payer must have approved at least amount plus the fee. The fee is
/// pinned: if the ledger's fee is no longer `fee` it refuses (BadFee)
/// rather than charging the payer something else.
pub async fn pull(
    ledger: Principal,
    payer: Principal,
    amount: u128,
    fee: u128,
) -> Result<u128, Failure> {
    call(
        ledger,
        "icrc2_transfer_from",
        TransferFromArgs {
            to: account(ic_cdk::api::canister_self()),
            fee: Some(Nat::from(fee)),
            spender_subaccount: None,
            from: account(payer),
            memo: None,
            created_at_time: None,
            amount: Nat::from(amount),
        },
    )
    .await
}

/// Pay `amount` cycles from this canister's ledger account to `to`. The
/// pinned `fee` comes out of this canister's account on top; a changed
/// fee makes the ledger refuse rather than debit more than was budgeted.
pub async fn pay(
    ledger: Principal,
    to: Principal,
    amount: u128,
    fee: u128,
) -> Result<u128, Failure> {
    call(
        ledger,
        "icrc1_transfer",
        TransferArgs {
            to: account(to),
            fee: Some(Nat::from(fee)),
            memo: None,
            from_subaccount: None,
            created_at_time: None,
            amount: Nat::from(amount),
        },
    )
    .await
}

/// Move `amount` cycles from this canister's ledger account into its own
/// cycles balance. `withdraw` has no fee field to pin, so the caller
/// checks the live fee against the pinned one first.
pub async fn fund_self(ledger: Principal, amount: u128) -> Result<u128, Failure> {
    call(
        ledger,
        "withdraw",
        WithdrawArgs {
            to: ic_cdk::api::canister_self(),
            from_subaccount: None,
            created_at_time: None,
            amount: Nat::from(amount),
        },
    )
    .await
}
