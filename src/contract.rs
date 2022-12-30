use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal256, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, Uint256,
};
use cw0::maybe_addr;

use crate::msg::{
    AccruedRewardsResponse, ExecuteMsg, HolderResponse, InstantiateMsg, MigrateMsg, QueryMsg,
    StateResponse,
};
use crate::state::{Holder, State, HOLDERS, STATE};
use crate::ContractError;

use std::convert::TryInto;
use std::ops::Add;
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
        total_staked: Uint128::zero(),
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
        ExecuteMsg::BondStake {} => execute_bond(deps, env, info),
        ExecuteMsg::UpdateRewardIndex {} => execute_update_reward_index(deps, env),
        ExecuteMsg::UpdateHoldersreward { address } => {
            execute_update_holders_rewards(deps, env, info, address)
        }
        ExecuteMsg::WithdrawStake { amount } => execute_withdraw(deps, env, info, amount),
        ExecuteMsg::ReceiveReward {} => execute_receive_reward(deps, env, info),
    }
}

/// Increase global_index according to claimed rewards amount
pub fn execute_update_reward_index(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    // Zero staking check
    if state.total_staked.is_zero() {
        return Err(ContractError::NoBond {});
    }
    let claimed_rewards = update_reward_index(&mut state, deps, env)?;

    // For querying the balance of the contract itself, we can use the querier

    let res = Response::new()
        .add_attribute("action", "update_reward_index")
        .add_attribute("claimed_rewards", claimed_rewards)
        .add_attribute("new_index", state.global_index.to_string());
    Ok(res)
}

pub fn update_reward_index(
    state: &mut State,
    mut deps: DepsMut,
    env: Env,
) -> Result<Uint128, ContractError> {
    let current_balance: Uint128 = deps
        .branch()
        .querier
        .query_balance(&env.contract.address, &state.reward_denom)?
        .amount;
    let previous_balance = state.prev_reward_balance;
    println!("current_balance: {}", current_balance);
    println!("previous_balance: {}", previous_balance);
    // claimed_rewards = current_balance - prev_balance;
    let claimed_rewards = current_balance.checked_sub(previous_balance)?;
    println!("claimed_rewards: {}", claimed_rewards);
    state.prev_reward_balance = current_balance;

    // global_index += claimed_rewards / total_balance;
    if !state.total_staked.is_zero() {
        state.global_index = state
            .global_index
            .add(Decimal256::from_ratio(claimed_rewards, state.total_staked));
    }

    STATE.save(deps.storage, &state)?;
    Ok(claimed_rewards)
}
//execute update rewards yazilacak
pub fn execute_update_holders_rewards(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: Option<String>,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    // Zero staking check
    if state.total_staked.is_zero() {
        return Err(ContractError::NoBond {});
    }
    //validate address
    let addr = maybe_addr(deps.api, address)?.unwrap_or(info.sender);
    let mut holder = HOLDERS.load(deps.storage, &Addr::unchecked(addr.clone()))?;
    update_holders_rewards(deps.branch(), env, &mut holder)?;
    HOLDERS.save(deps.storage, &Addr::unchecked(addr), &holder)?;

    let res = Response::new()
        .add_attribute("action", "update_reward_index")
        .add_attribute("pending_rewards", holder.pending_rewards)
        .add_attribute("new_index", state.global_index.to_string())
        .add_attribute("holders index", holder.index.to_string());
    Ok(res)
}

pub fn update_holders_rewards(
    mut deps: DepsMut,
    env: Env,
    holder: &mut Holder,
) -> Result<Uint128, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    //update reward index
    update_reward_index(&mut state, deps.branch(), env)?;

    let rewards_uint128;
    //index_diff = global_index - holder.index;
    let index_diff: Decimal256 = state.global_index - holder.index;
    //reward_amount = holder.balance * index_diff + holder.pending_rewards;
    let reward_amount = Decimal256::from_ratio(holder.balance, Uint256::one())
        .checked_mul(index_diff)?
        .checked_add(holder.dec_rewards)?;
    //
    let decimals = get_decimals(reward_amount)?;

    //floor(reward_amount)
    rewards_uint128 = (reward_amount * Uint256::one())
        .try_into()
        .unwrap_or(Uint128::zero());
    holder.dec_rewards = decimals;
    holder.pending_rewards = rewards_uint128;
    holder.index = state.global_index;
    Ok(rewards_uint128)
}

pub fn execute_receive_reward(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    let mut holder = HOLDERS.load(deps.storage, &Addr::unchecked(info.sender.as_str()))?;
    if holder.balance.is_zero() {
        return Err(ContractError::NoBond {});
    }
    let rewards_uint128 = update_holders_rewards(deps.branch(), env, &mut holder)?;

    HOLDERS.save(
        deps.storage,
        &Addr::unchecked(info.sender.as_str()),
        &holder,
    )?;

    //send rewards to the holder

    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: state.reward_denom.to_string(),
            amount: rewards_uint128,
        }],
    });

    if rewards_uint128.is_zero() {
        return Err(ContractError::NoRewards {});
    }

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "receive_reward")
        .add_attribute("rewards", rewards_uint128)
        .add_attribute("holder", info.sender)
        .add_attribute("holder_balance", holder.balance))
}

pub fn execute_bond(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    let addr = info.sender;
    let amount = info
        .funds
        .iter()
        .find(|coin| coin.denom == state.staked_token_denom)
        .map(|coin| coin.amount)
        .unwrap_or(Uint128::zero());

    if amount.is_zero() {
        return Err(ContractError::NoFund {});
    }

    let holder = HOLDERS.may_load(deps.storage, &addr)?;

    match holder {
        None => {
            update_reward_index(&mut state, deps.branch(), env)?;
            let holder = Holder::new(
                amount,
                state.global_index,
                Uint128::zero(),
                Decimal256::zero(),
            );

            HOLDERS.save(deps.storage, &addr, &holder)?;
        }
        Some(mut holder) => {
            update_reward_index(&mut state, deps.branch(), env.clone())?;
            if holder.balance.is_zero() {
                return Err(ContractError::NoBond {});
            }

            update_holders_rewards(deps.branch(), env.clone(), &mut holder)?;
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

    if !info.funds.is_empty() {
        return Err(ContractError::DoNotSendFunds {});
    }
    //amount to optional

    let mut holder = HOLDERS.load(deps.storage, &info.sender)?;
    let withdraw_amount = amount.unwrap_or(holder.balance);

    if holder.balance < withdraw_amount {
        return Err(ContractError::DecreaseAmountExceeds(holder.balance));
    }

    update_holders_rewards(deps.branch(), env.clone(), &mut holder)?;
    update_reward_index(&mut state, deps.branch(), env.clone())?;

    //send rewards and withdraw amount to the holder
    let res: Response = Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![
                Coin {
                    denom: state.reward_denom.to_string(),
                    amount: holder.pending_rewards,
                },
                Coin {
                    denom: state.staked_token_denom.to_string(),
                    amount: withdraw_amount,
                },
            ],
        }))
        .add_attribute("action", "unbond_stake")
        .add_attribute("holder_address", info.sender.clone())
        .add_attribute("amount", withdraw_amount);

    holder.balance = (holder.balance.checked_sub(withdraw_amount))?;
    state.total_staked = (state.total_staked.checked_sub(withdraw_amount))?;
    STATE.save(deps.storage, &state)?;
    HOLDERS.save(deps.storage, &info.sender, &holder)?;
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: DepsMut, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => to_binary(&query_state(deps, env, msg)?),
        QueryMsg::AccruedRewards { address } => {
            to_binary(&query_accrued_rewards(env, deps, address)?)
        }
        QueryMsg::Holder { address } => to_binary(&query_holder(env, deps, address)?),
        // QueryMsg::Holders { start_after, limit } => {
        //     to_binary(&query_holders(deps, start_after, limit)?)
        // } // QueryMsg::Claims { address } => to_binary(&query_claims(deps, address)?),
    }
}

pub fn query_state(deps: DepsMut, _env: Env, _msg: QueryMsg) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;

    Ok(StateResponse {
        staked_token_denom: state.staked_token_denom,
        reward_denom: state.reward_denom,
        total_staked: state.total_staked,
        global_index: state.global_index,
        prev_reward_balance: state.prev_reward_balance,
    })
}

pub fn query_accrued_rewards(
    _env: Env,
    deps: DepsMut,
    address: String,
) -> StdResult<AccruedRewardsResponse> {
    let addr = deps.api.addr_validate(&address.as_str())?;
    let holder = HOLDERS.load(deps.storage, &addr)?;
    let _state = STATE.load(deps.storage)?;

    Ok(AccruedRewardsResponse {
        rewards: holder.pending_rewards,
    })
}

pub fn query_holder(_env: Env, deps: DepsMut, address: String) -> StdResult<HolderResponse> {
    let holder: Holder = HOLDERS.load(deps.storage, &deps.api.addr_validate(address.as_str())?)?;
    let _state = STATE.load(deps.storage)?;
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
