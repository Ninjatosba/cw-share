use cosmwasm_std::{OverflowError, StdError, Uint128};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("No rewards accrued")]
    NoRewards {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Do not send native funds")]
    DoNotSendFunds {},

    #[error("Amount required")]
    AmountRequired {},

    #[error("Decrease amount exceeds user balance: {0}")]
    DecreaseAmountExceeds(Uint128),

    #[error("No bond")]
    NoBond {},

    #[error("Please send right denom and funds")]
    NoFund {},

    #[error("Address validation failed")]
    InvalidAddress {},

    #[error("Stake denom and reward denom cannot be same")]
    SameDenom {},
}
