use cosmwasm_std::{Addr, Decimal256, Uint128};

use cw_controllers::Claims;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub global_index: Decimal256,
    pub total_staked: Uint128,
    pub total_rewards: Uint128,
    pub rewards_claimed: Uint128,
}
pub const STATE: Item<State> = Item::new("state");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub staked_token_denom: String,
    pub reward_denom: String,
    pub admin: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Holder {
    pub balance: Uint128,
    pub index: Decimal256,
    pub dec_rewards: Decimal256,
    pub pending_rewards: Uint128,
}

// REWARDS (holder_addr, cw20_addr) -> Holder
pub const HOLDERS: Map<&Addr, Holder> = Map::new("holders");

pub const CLAIMS: Claims = Claims::new("claims");

impl Holder {
    pub fn new(
        balance: Uint128,
        index: Decimal256,
        pending_rewards: Uint128,
        dec_rewards: Decimal256,
    ) -> Self {
        Holder {
            balance,
            index,
            pending_rewards,
            dec_rewards,
        }
    }
}
