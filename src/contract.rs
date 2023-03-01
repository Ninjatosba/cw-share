use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal256, Deps, DepsMut, Env,
    MessageInfo, Order, Response, StdError, StdResult, Uint128, Uint256,
};
use cw0::maybe_addr;
use cw_storage_plus::Bound;
use cw_utils::must_pay;

use crate::msg::{
    AccruedRewardsResponse, ConfigResponse, ExecuteMsg, HolderResponse, HoldersResponse,
    InstantiateMsg, MigrateMsg, QueryMsg, StateResponse,
};
use crate::state::{Config, Holder, State, CONFIG, HOLDERS, STATE};
use crate::ContractError;

use std::convert::TryInto;
use std::ops::Add;
use std::str::FromStr;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    //check if admin is a valid address and if it is, set it to the admin field else set it as sender
    let admin = match msg.admin {
        Some(admin) => deps.api.addr_validate(&admin)?,
        None => info.sender.clone(),
    };

    //check if staked token denom is same as reward denom
    if msg.staked_token_denom == msg.reward_denom {
        return Err(ContractError::SameDenom {});
    }

    let config: Config = Config {
        staked_token_denom: msg.staked_token_denom,
        reward_denom: msg.reward_denom,
        admin: admin,
    };

    let state = State {
        global_index: Decimal256::zero(),
        total_staked: Uint128::zero(),
        total_rewards: Uint128::zero(),
        rewards_claimed: Uint128::zero(),
    };

    CONFIG.save(deps.storage, &config)?;

    STATE.save(deps.storage, &state)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateReward {} => execute_update_reward(deps, env, info),
        ExecuteMsg::BondStake {} => execute_bond(deps, env, info),
        ExecuteMsg::UpdateHolderReward { address } => {
            execute_update_holder_rewards(deps, env, info, address)
        }
        ExecuteMsg::WithdrawStake { amount } => execute_withdraw(deps, env, info, amount),
        ExecuteMsg::ReceiveReward {} => execute_receive_reward(deps, env, info),
        ExecuteMsg::AdminWithdrawAll {} => execute_admin_withdraw_all(deps, env, info),
    }
}

pub fn execute_update_reward(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    let config = CONFIG.load(deps.storage)?;

    // Check funds
    let amount = must_pay(&info, &config.reward_denom)?;

    // update index
    state.global_index = state
        .global_index
        .checked_add(Decimal256::from_ratio(amount, state.total_staked))?;

    state.total_rewards = state.total_rewards.add(amount);

    STATE.save(deps.storage, &state)?;

    let res = Response::new()
        .add_attribute("action", "update_reward")
        .add_attribute("reward", amount.to_string());
    Ok(res)
}

pub fn execute_update_holder_rewards(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: Option<String>,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    // Zero staking check
    if state.total_staked.is_zero() {
        return Err(ContractError::NoBond {});
    }
    //validate address
    let addr = maybe_addr(deps.api, address)?.unwrap_or(info.sender);
    let mut holder = HOLDERS.load(deps.storage, &addr)?;
    update_holder_rewards(deps.branch(), &mut state, env, &mut holder)?;
    HOLDERS.save(deps.storage, &Addr::unchecked(addr), &holder)?;
    STATE.save(deps.storage, &state)?;

    let res = Response::new()
        .add_attribute("action", "update_reward_index")
        .add_attribute("pending_rewards", holder.pending_rewards)
        .add_attribute("new_index", state.global_index.to_string())
        .add_attribute("holders index", holder.index.to_string());
    Ok(res)
}

pub fn update_holder_rewards(
    mut deps: DepsMut,
    state: &mut State,
    env: Env,
    holder: &mut Holder,
) -> Result<Uint128, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let rewards_uint128;

    //index_diff = global_index - holder.index;
    let index_diff: Decimal256 = state.global_index - holder.index;

    //reward_amount = holder.balance * index_diff + holder.pending_rewards;
    let reward_amount = Decimal256::from_ratio(holder.balance, Uint256::one())
        .checked_mul(index_diff)?
        .checked_add(holder.dec_rewards)?;
    let decimals = get_decimals(reward_amount)?;

    //floor(reward_amount)
    rewards_uint128 = (reward_amount * Uint256::one())
        .try_into()
        .unwrap_or(Uint128::zero());

    holder.dec_rewards = decimals;

    holder.pending_rewards += rewards_uint128;

    holder.index = state.global_index;

    Ok(rewards_uint128)
}

pub fn execute_receive_reward(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    let mut holder = HOLDERS.load(deps.storage, &info.sender)?;

    if holder.balance.is_zero() {
        return Err(ContractError::NoBond {});
    }

    update_holder_rewards(deps.branch(), &mut state, env, &mut holder)?;

    if holder.pending_rewards.is_zero() {
        return Err(ContractError::NoRewards {});
    }
    //send rewards to the holder
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.reward_denom.to_string(),
            amount: holder.pending_rewards,
        }],
    });
    state.rewards_claimed += holder.pending_rewards;

    holder.pending_rewards = Uint128::zero();

    HOLDERS.save(deps.storage, &info.sender, &holder)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "receive_reward")
        .add_attribute("rewards", holder.pending_rewards)
        .add_attribute("holder", info.sender)
        .add_attribute("holder_balance", holder.balance))
}

pub fn execute_bond(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    //check if denom sent is the same as the staked token else return error
    let amount = must_pay(&info, &config.staked_token_denom)?;
    let addr = info.sender;

    let holder = HOLDERS.may_load(deps.storage, &addr)?;

    match holder {
        None => {
            let holder = Holder::new(
                amount,
                state.global_index,
                Uint128::zero(),
                Decimal256::zero(),
            );

            HOLDERS.save(deps.storage, &addr, &holder)?;
        }
        Some(mut holder) => {
            update_holder_rewards(deps.branch(), &mut state, env, &mut holder)?;
            holder.balance += amount;

            HOLDERS.save(deps.storage, &addr, &holder)?;
        }
    }
    state.total_staked += amount;
    STATE.save(deps.storage, &state)?;

    let res = Response::new()
        .add_attribute("action", "bond_stake")
        .add_attribute("holder_address", addr)
        .add_attribute("amount", amount);

    Ok(res)
}

pub fn execute_withdraw(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;
    if !info.funds.is_empty() {
        return Err(ContractError::DoNotSendFunds {});
    }

    let mut holder = HOLDERS.load(deps.storage, &info.sender)?;
    let withdraw_amount = amount.unwrap_or(holder.balance);

    if holder.balance < withdraw_amount {
        return Err(ContractError::DecreaseAmountExceeds(holder.balance));
    }

    update_holder_rewards(deps.branch(), &mut state, env.clone(), &mut holder)?;

    //send rewards and withdraw amount to the holder
    let res: Response = Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: config.reward_denom.to_string(),
                amount: holder.pending_rewards,
            }],
        }))
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: config.staked_token_denom.to_string(),
                amount: withdraw_amount,
            }],
        }))
        .add_attribute("action", "withdraw_stake")
        .add_attribute("holder_address", info.sender.clone())
        .add_attribute("amount", withdraw_amount)
        .add_attribute("rewards claimed", holder.pending_rewards);

    holder.balance = (holder.balance.checked_sub(withdraw_amount))?;
    state.total_staked = (state.total_staked.checked_sub(withdraw_amount))?;
    holder.pending_rewards = Uint128::zero();
    STATE.save(deps.storage, &state)?;
    HOLDERS.save(deps.storage, &info.sender, &holder)?;
    Ok(res)
}

pub fn execute_admin_withdraw_all(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    // all tokens
    let all_tokens = deps
        .branch()
        .querier
        .query_all_balances(&env.contract.address)?;

    let res = Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: all_tokens,
        }))
        .add_attribute("action", "admin_withdraw_all")
        .add_attribute("admin_address", info.sender.clone());
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => to_binary(&query_state(deps, env, msg)?),
        QueryMsg::AccruedRewards { address } => {
            to_binary(&query_accrued_rewards(env, deps, address)?)
        }
        QueryMsg::Holder { address } => to_binary(&query_holder(env, deps, address)?),
        QueryMsg::Config {} => to_binary(&query_config(deps, env, msg)?),
        QueryMsg::Holders { start_after, limit } => {
            to_binary(&query_holders(deps, env, start_after, limit)?)
        }
    }
}

pub fn query_state(deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;

    Ok(StateResponse {
        total_staked: state.total_staked,
        global_index: state.global_index,
        total_rewards: state.total_rewards,
        rewards_claimed: state.rewards_claimed,
    })
}

//query config
pub fn query_config(deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        staked_token_denom: config.staked_token_denom,
        reward_denom: config.reward_denom,
        admin: config.admin.into_string(),
    })
}

pub fn query_accrued_rewards(
    _env: Env,
    deps: Deps,
    address: String,
) -> StdResult<AccruedRewardsResponse> {
    let addr = deps.api.addr_validate(&address.as_str())?;
    let holder = HOLDERS.load(deps.storage, &addr)?;

    Ok(AccruedRewardsResponse {
        rewards: holder.pending_rewards,
    })
}

pub fn query_holder(_env: Env, deps: Deps, address: String) -> StdResult<HolderResponse> {
    let holder: Holder = HOLDERS.load(deps.storage, &deps.api.addr_validate(address.as_str())?)?;
    Ok(HolderResponse {
        address: address,
        balance: holder.balance,
        index: holder.index,
        pending_rewards: holder.pending_rewards,
        dec_rewards: holder.dec_rewards,
    })
}

// calculate the reward with decimal
pub fn get_decimals(value: Decimal256) -> StdResult<Decimal256> {
    let stringed: &str = &*value.to_string();
    let parts: &[&str] = &*stringed.split('.').collect::<Vec<&str>>();
    match parts.len() {
        1 => Ok(Decimal256::zero()),
        2 => {
            let decimals: Decimal256 = Decimal256::from_str(&*("0.".to_owned() + parts[1]))?;
            Ok(decimals)
        }
        _ => Err(StdError::generic_err("Unexpected number of dots")),
    }
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
//query all holders list
pub fn query_holders(
    deps: Deps,
    _env: Env,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<HoldersResponse> {
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.as_ref().map(Bound::exclusive);
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let holders: StdResult<Vec<HolderResponse>> = HOLDERS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (addr, holder) = item?;
            let holder_response = HolderResponse {
                address: addr.to_string(),
                balance: holder.balance,
                index: holder.index,
                pending_rewards: holder.pending_rewards,
                dec_rewards: holder.dec_rewards,
            };
            Ok(holder_response)
        })
        .collect();

    Ok(HoldersResponse { holders: holders? })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
