use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal256, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, Uint256,
};
use cw0::maybe_addr;
use cw_utils::must_pay;

use crate::msg::{
    AccruedRewardsResponse, ConfigResponse, ExecuteMsg, HolderResponse, InstantiateMsg, MigrateMsg,
    QueryMsg, StateResponse,
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
) -> StdResult<Response> {
    //check if admin is a valid address and if it is, set it to the admin field else set it as sender
    let admin = match msg.admin {
        Some(admin) => deps.api.addr_validate(&admin)?,
        None => info.sender.clone(),
    };

    let config: Config = Config {
        staked_token_denom: msg.staked_token_denom,
        reward_denom: msg.reward_denom,
        admin: admin,
    };
    let state = State {
        global_index: Decimal256::zero(),
        total_staked: Uint128::zero(),
        prev_reward_balance: Uint128::zero(),
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
        ExecuteMsg::BondStake {} => execute_bond(deps, env, info),
        ExecuteMsg::UpdateRewardIndex {} => execute_update_reward_index(deps, env),
        ExecuteMsg::UpdateHoldersReward { address } => {
            execute_update_holders_rewards(deps, env, info, address)
        }
        ExecuteMsg::WithdrawStake { amount } => execute_withdraw(deps, env, info, amount),
        ExecuteMsg::ReceiveReward {} => execute_receive_reward(deps, env, info),
        ExecuteMsg::UpdateConfig {
            staked_token_denom,
            reward_denom,
            admin,
        } => execute_update_config(deps, env, info, staked_token_denom, reward_denom, admin),
    }
}

/// Increase global_index according to claimed rewards amount
pub fn execute_update_reward_index(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    // Zero staking check
    if state.total_staked.is_zero() {
        return Err(ContractError::NoBond {});
    }

    let claimed_rewards = update_reward_index(&mut state, config, deps, env)?;

    // For querying the balance of the contract itself, we can use the querier

    let res = Response::new()
        .add_attribute("action", "update_reward_index")
        .add_attribute("claimed_rewards", claimed_rewards)
        .add_attribute("new_index", state.global_index.to_string());
    Ok(res)
}

pub fn update_reward_index(
    state: &mut State,
    config: Config,
    mut deps: DepsMut,
    env: Env,
) -> Result<Uint128, ContractError> {
    let current_balance: Uint128 = deps
        .branch()
        .querier
        .query_balance(&env.contract.address, &config.reward_denom)?
        .amount;
    if current_balance >= state.prev_reward_balance {
        let previous_balance = state.prev_reward_balance;
        // claimed_rewards = current_balance - prev_balance;
        let claimed_rewards = current_balance.checked_sub(previous_balance)?;

        state.prev_reward_balance = current_balance;

        // global_index += claimed_rewards / total_balance;
        if !state.total_staked.is_zero() {
            state.global_index = state
                .global_index
                .add(Decimal256::from_ratio(claimed_rewards, state.total_staked));
        }

        STATE.save(deps.storage, &state)?;
        Ok(claimed_rewards)
    } else {
        //this means that the some users recieved rewards and the contract balance has decreased

        state.prev_reward_balance = current_balance;
        STATE.save(deps.storage, &state)?;
        Ok(current_balance)
    }
}

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
    let config = CONFIG.load(deps.storage)?;

    //update reward index
    update_reward_index(&mut state, config, deps.branch(), env)?;

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
    let _state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    let mut holder = HOLDERS.load(deps.storage, &Addr::unchecked(info.sender.as_str()))?;
    if holder.balance.is_zero() {
        return Err(ContractError::NoBond {});
    }
    update_holders_rewards(deps.branch(), env, &mut holder)?;

    HOLDERS.save(
        deps.storage,
        &Addr::unchecked(info.sender.as_str()),
        &holder,
    )?;

    //send rewards to the holder
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.reward_denom.to_string(),
            amount: holder.pending_rewards,
        }],
    });

    if holder.pending_rewards.is_zero() {
        return Err(ContractError::NoRewards {});
    }

    holder.pending_rewards = Uint128::zero();

    HOLDERS.save(
        deps.storage,
        &Addr::unchecked(info.sender.as_str()),
        &holder,
    )?;
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

    if amount.is_zero() {
        return Err(ContractError::NoFund {});
    }

    let holder = HOLDERS.may_load(deps.storage, &addr)?;

    match holder {
        None => {
            update_reward_index(&mut state, config, deps.branch(), env)?;
            let holder = Holder::new(
                amount,
                state.global_index,
                Uint128::zero(),
                Decimal256::zero(),
            );

            HOLDERS.save(deps.storage, &addr, &holder)?;
        }
        Some(mut holder) => {
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
    let config = CONFIG.load(deps.storage)?;
    if !info.funds.is_empty() {
        return Err(ContractError::DoNotSendFunds {});
    }

    let mut holder = HOLDERS.load(deps.storage, &info.sender)?;
    let withdraw_amount = amount.unwrap_or(holder.balance);

    if holder.balance < withdraw_amount {
        return Err(ContractError::DecreaseAmountExceeds(holder.balance));
    }

    update_holders_rewards(deps.branch(), env.clone(), &mut holder)?;
    update_reward_index(&mut state, config.clone(), deps.branch(), env.clone())?;

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

//update config
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    staked_token_denom: Option<String>,
    reward_denom: Option<String>,
    admin: Option<String>,
) -> Result<Response, ContractError> {
    let old_config: Config = CONFIG.load(deps.storage)?;

    //check if admin is an valid address and set admin
    let admin = match admin {
        Some(admin) => deps.api.addr_validate(&admin)?,
        None => old_config.clone().admin,
    };

    if info.sender != old_config.clone().admin {
        return Err(ContractError::Unauthorized {});
    };

    let config = Config {
        staked_token_denom: staked_token_denom.unwrap_or(old_config.staked_token_denom),
        reward_denom: reward_denom.unwrap_or(old_config.reward_denom),
        admin,
    };

    CONFIG.save(deps.storage, &config)?;

    let res = Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("staked_token_denom", config.staked_token_denom)
        .add_attribute("reward_denom", config.reward_denom)
        .add_attribute("admin", config.admin);

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
    }
}

pub fn query_state(deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;

    Ok(StateResponse {
        total_staked: state.total_staked,
        global_index: state.global_index,
        prev_reward_balance: state.prev_reward_balance,
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
