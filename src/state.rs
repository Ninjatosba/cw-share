use crate::msg::HolderResponse;
use cosmwasm_std::{
    Addr, Api, CanonicalAddr, Decimal, Decimal256, Deps, DepsMut, Order, StdResult, Uint128,
};
use cw20::Balance;
use cw_controllers::Claims;
use cw_storage_plus::{Bound, Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub global_index: Decimal256,
    pub total_staked: Uint128,
    pub prev_reward_balance: Uint128,
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

/// list_accrued_rewards settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
// pub fn list_accrued_rewards(
//     deps: DepsMut,
//     start_after: Option<String>,
//     limit: Option<u32>,
// ) -> StdResult<Vec<HolderResponse>> {
//     let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
//     let addr = maybe_addr(deps.api, start_after)?;
//     let start = addr.as_ref().map(Bound::exclusive);

//     HOLDERS
//         .range(deps.storage, start, None, Order::Ascending)
//         .take(limit)
//         .map(|elem| {
//             let (addr, v) = elem?;
//             Ok(HolderResponse {
//                 address: addr.to_string(),
//                 balance: v.balance,
//                 index: v.index,
//                 pending_rewards: v.pending_rewards,
//                 dec_rewards: v.dec_rewards,
//             })
//         })
//         .collect()
// }

fn calc_range_start(api: &dyn Api, start_after: Option<Addr>) -> StdResult<Option<Vec<u8>>> {
    match start_after {
        Some(human) => {
            let mut v: Vec<u8> = api.addr_canonicalize(human.as_ref())?.0.into();
            v.push(0);
            Ok(Some(v))
        }
        None => Ok(None),
    }
}

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
