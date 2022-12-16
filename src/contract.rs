use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Decimal256,
    Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128, Uint256, WasmMsg,
};

use crate::msg::{
    AccruedRewardsResponse, ExecuteMsg, HolderResponse, HoldersResponse, InstantiateMsg,
    MigrateMsg, QueryMsg, StateResponse,
};
use crate::state::{list_accrued_rewards, Holder, State, CLAIMS, HOLDERS, STATE};
use crate::ContractError;
use cw_controllers::ClaimsResponse;
use std::convert::TryInto;
use std::ops::{Add, Mul, Sub};
use std::str::FromStr;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let state = State {
        staked_token_denom: msg.staked_token_denom,
        reward_denom: msg.reward_denom,
        global_index: Decimal256::zero(),
        total_balance: Uint128::zero(),
        prev_reward_balance: Uint128::zero(),
    };
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
        ExecuteMsg::UpdateRewardIndex {} => execute_update_reward_index(deps, env),
        ExecuteMsg::UnbondStake { amount } => execute_unbound(deps, env, info, amount),
        ExecuteMsg::WithdrawStake { cap } => execute_withdraw_stake(deps, env, info, cap),
        ExecuteMsg::ReceiveReward {} => execute_receive_reward(deps, env, info),
    }
}

/// Increase global_index according to claimed rewards amount
pub fn execute_update_reward_index(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    // Zero staking balance check
    if state.total_balance.is_zero() {
        return Err(ContractError::NoBond {});
    }
    let claimed_rewards = update_reward_index(&mut state, deps, env)?;

    // For querying the balance of the contract itself, we can use the querier
    // We should find a way to add other token denoms for trasuary
    // For that we should store more than one state with state id

    let res = Response::new()
        .add_attribute("action", "update_reward_index")
        .add_attribute("claimed_rewards", claimed_rewards)
        .add_attribute("new_index", state.global_index.to_string());
    Ok(res)
}

pub fn update_reward_index(
    state: &mut State,
    deps: DepsMut,
    env: Env,
) -> Result<Uint128, ContractError> {
    let current_balance: Uint128 = deps
        .querier
        .query_balance(&env.contract.address, &state.reward_denom)?
        .amount;
    let previous_balance = state.prev_reward_balance;

    // claimed_rewards = current_balance - prev_balance;
    let claimed_rewards = current_balance.checked_sub(previous_balance)?;

    state.prev_reward_balance = current_balance;

    // global_index += claimed_rewards / total_balance;
    state.global_index = state
        .global_index
        .add(Decimal256::from_ratio(claimed_rewards, state.total_balance));

    STATE.save(deps.storage, &state)?;
    Ok(claimed_rewards)
}

pub fn execute_receive_reward(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    let mut holder = HOLDERS.load(deps.storage, &Addr::unchecked(info.sender.as_str()))?;
    if holder.balance.is_zero() {
        return Err(ContractError::NoBond {});
    }
    // update reward index
    update_reward_index(&mut state, deps, env)?;

    let mut rewards_uint128 = Uint128::zero();
    //index_diff = global_index - holder.index;
    let index_diff: Decimal256 = state.global_index - holder.index;
    //reward_amount = holder.balance * index_diff + holder.pending_rewards;
    let reward_amount = Decimal256::from_ratio(holder.balance, Uint256::one())
        .checked_mul(index_diff)?
        .checked_add(holder.pending_rewards)?;

    let decimals = get_decimals(reward_amount)?;

    //floor(reward_amount)
    rewards_uint128 = (reward_amount * Uint256::one()).try_into()?;
    holder.pending_rewards = decimals;

    //send rewards to the holder
    let res = Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: state.reward_denom.to_string(),
                amount: rewards_uint128,
            }],
        }))
        .add_attribute("action", "receive_reward")
        .add_attribute("rewards", rewards_uint128);

    Ok(res)
}

pub fn execute_bond(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    holder_addr: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::DoNotSendFunds {});
    }
    if amount.is_zero() {
        return Err(ContractError::AmountRequired {});
    }

    let addr = deps.api.addr_validate(&holder_addr.as_str())?;
    let mut state = STATE.load(deps.storage)?;

    let mut holder = HOLDERS.may_load(deps.storage, &addr)?.unwrap_or(Holder {
        balance: Uint128::zero(),
        index: Decimal::zero(),
        pending_rewards: Decimal::zero(),
    });

    // get decimals
    //in new bonding rewards=global_index*balance
    let rewards = calculate_decimal_rewards(state.global_index, holder.index, holder.balance)?;

    holder.index = state.global_index;
    holder.pending_rewards = rewards.sub(holder.pending_rewards);
    holder.balance = amount;
    // save reward and index
    HOLDERS.save(deps.storage, &addr, &holder)?;

    state.total_balance += amount;
    STATE.save(deps.storage, &state)?;

    let res = Response::new()
        .add_attribute("action", "bond_stake")
        .add_attribute("holder_address", holder_addr)
        .add_attribute("amount", amount);

    Ok(res)
}

pub fn execute_unbound(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    if !info.funds.is_empty() {
        return Err(ContractError::DoNotSendFunds {});
    }
    if amount.is_zero() {
        return Err(ContractError::AmountRequired {});
    }

    let mut holder = HOLDERS.load(deps.storage, &info.sender)?;
    if holder.balance < amount {
        return Err(ContractError::DecreaseAmountExceeds(holder.balance));
    }

    let rewards = calculate_decimal_rewards(state.global_index, holder.index, holder.balance)?;

    holder.index = state.global_index;
    holder.pending_rewards = rewards.add(holder.pending_rewards);
    holder.balance = (holder.balance.checked_sub(amount))?;
    state.total_balance = (state.total_balance.checked_sub(amount))?;

    STATE.save(deps.storage, &state)?;
    HOLDERS.save(deps.storage, &info.sender, &holder)?;

    let attributes = vec![
        attr("action", "unbound"),
        attr("holder_address", info.sender),
        attr("amount", amount),
    ];

    Ok(Response::new().add_attributes(attributes))
}

pub fn execute_withdraw_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cap: Option<Uint128>,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    let amount = CLAIMS.claim_tokens(deps.storage, &info.sender, &env.block, cap)?;
    if amount.is_zero() {
        return Err(ContractError::WaitUnbonding {});
    }

    let cw20_transfer_msg = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.to_string(),
        amount,
    };
    let msg = WasmMsg::Execute {
        contract_addr: state.token_address,
        msg: to_binary(&cw20_transfer_msg)?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "withdraw_stake")
        .add_attribute("holder_address", &info.sender)
        .add_attribute("amount", amount))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => to_binary(&query_state(deps, _env, msg)?),
        QueryMsg::AccruedRewards { address } => to_binary(&query_accrued_rewards(deps, address)?),
        QueryMsg::Holder { address } => to_binary(&query_holder(deps, address)?),
        QueryMsg::Holders { start_after, limit } => {
            to_binary(&query_holders(deps, start_after, limit)?)
        }
        QueryMsg::Claims { address } => to_binary(&query_claims(deps, address)?),
    }
}

pub fn query_state(deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;

    Ok(StateResponse {
        token_address: state.token_address,
        global_index: state.global_index,
        total_balance: state.total_balance,
        prev_reward_balance: state.prev_reward_balance,
    })
}

pub fn query_accrued_rewards(deps: Deps, address: String) -> StdResult<AccruedRewardsResponse> {
    let state = STATE.load(deps.storage)?;

    let addr = deps.api.addr_validate(address.as_str())?;
    let holder = HOLDERS.load(deps.storage, &addr)?;
    let reward_with_decimals =
        calculate_decimal_rewards(state.global_index, holder.index, holder.balance)?;
    let all_reward_with_decimals = reward_with_decimals.add(holder.pending_rewards);

    let rewards = all_reward_with_decimals * Uint128::new(1);

    Ok(AccruedRewardsResponse { rewards })
}

pub fn query_holder(deps: Deps, address: String) -> StdResult<HolderResponse> {
    let holder: Holder = HOLDERS.load(deps.storage, &deps.api.addr_validate(address.as_str())?)?;
    Ok(HolderResponse {
        address,
        balance: holder.balance,
        index: holder.index,
        pending_rewards: holder.pending_rewards,
    })
}

pub fn query_holders(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<HoldersResponse> {
    let start_after = if let Some(start_after) = start_after {
        Some(deps.api.addr_validate(&start_after)?)
    } else {
        None
    };

    let holders: Vec<HolderResponse> = list_accrued_rewards(deps, start_after, limit)?;

    Ok(HoldersResponse { holders })
}

pub fn query_claims(deps: Deps, addr: String) -> StdResult<ClaimsResponse> {
    Ok(CLAIMS.query_claims(deps, &deps.api.addr_validate(addr.as_str())?)?)
}

// calculate the reward based on the sender's index and the global index.
pub fn calculate_decimal_rewards(
    global_index: Decimal,
    user_index: Decimal,
    user_balance: Uint128,
) -> StdResult<Decimal> {
    let decimal_balance = Decimal::from_ratio(user_balance, Uint128::new(1));

    Ok(global_index.sub(user_index).mul(decimal_balance))
}

// calculate the reward with decimal
pub fn get_decimals(value: Decimal256) -> StdResult<Decimal256> {
    let stringed: &str = &*value.to_string();
    let parts: &[&str] = &*stringed.split('.').collect::<Vec<&str>>();
    match parts.len() {
        1 => Ok(Decimal::zero()),
        2 => {
            let decimals = Decimal::from_str(&*("0.".to_owned() + parts[1]))?;
            Ok(decimals)
        }
        _ => Err(StdError::generic_err("Unexpected number of dots")),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
