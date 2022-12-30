#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_dependencies_with_balances,
        mock_env, mock_info,
    };
    use cosmwasm_std::{
        coin, from_binary, to_binary, Addr, BankMsg, BankQuery, Coin, CosmosMsg, Decimal,
        Decimal256, Empty, MessageInfo, SubMsg, Uint128, Uint256, WasmMsg,
    };
    use schemars::_private::NoSerialize;

    use crate::contract::{execute, execute_bond, get_decimals, instantiate, query};
    use crate::msg::{
        ExecuteMsg, HolderResponse, HoldersResponse, InstantiateMsg, QueryMsg, ReceiveMsg,
        StateResponse,
    };

    use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

    use crate::state::{Holder, State, HOLDERS, STATE};
    use crate::ContractError;
    use cw_multi_test::{App, Contract, ContractWrapper};
    use std::borrow::BorrowMut;
    use std::ops::{Mul, Sub};
    use std::str::FromStr;

    // fn mock_app() -> App {
    //     App::default()
    // }

    // pub fn contract_cw20_reward() -> Box<dyn Contract<Empty>> {
    //     let contract = ContractWrapper::new(execute, instantiate, query);
    //     Box::new(contract)
    // }

    // pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    //     let contract = ContractWrapper::new(
    //         cw20_base::contract::execute,
    //         cw20_base::contract::instantiate,
    //         cw20_base::contract::query,
    //     );
    //     Box::new(contract)
    // }

    fn default_init() -> InstantiateMsg {
        InstantiateMsg {
            staked_token_denom: "staked".to_string(),
            reward_denom: "rewards".to_string(),
        }
    }

    // fn receive_stake_msg(sender: &str, amount: u128) -> ExecuteMsg {
    //     let bond_msg = ReceiveMsg::BondStake {};
    //     let cw20_receive_msg = Cw20ReceiveMsg {
    //         sender: sender.to_string(),
    //         amount: Uint128::new(amount),
    //         msg: to_binary(&bond_msg).unwrap(),
    //     };
    //     ExecuteMsg::Receive(cw20_receive_msg)
    // }

    #[test]
    fn proper_init() {
        let mut deps = mock_dependencies();
        let init_msg = default_init();
        let env = mock_env();
        let info = MessageInfo {
            sender: Addr::unchecked("ok"),
            funds: vec![],
        };
        let res = instantiate(deps.as_mut(), env.clone(), info, init_msg).unwrap();
        //default response attributes is empty
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(
            config_response,
            StateResponse {
                reward_denom: "rewards".to_string(),
                staked_token_denom: "staked".to_string(),
                global_index: Decimal256::zero(),
                total_staked: Uint128::zero(),
                prev_reward_balance: Uint128::zero(),
            }
        );
    }

    #[test]
    pub fn test_bond() {
        let mut deps = mock_dependencies();
        let init_msg = default_init();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), env.clone(), info.clone(), init_msg).unwrap();

        let info = mock_info("staker1", &[]);
        //bond with no fund
        let msg = ExecuteMsg::BondStake {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::NoFund {});
        //first bond
        let info = mock_info(
            "staker1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //query holder
        let res = query(
            deps.as_mut(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker1".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();
        assert_eq!(
            holder_response,
            HolderResponse {
                address: "staker1".to_string(),
                balance: Uint128::new(100),
                index: Decimal256::zero(),
                pending_rewards: Uint128::zero(),
                dec_rewards: Decimal256::zero(),
            }
        );
    }

    //test execute update reward index
    #[test]
    pub fn test_update_reward_index() {
        let mut deps = mock_dependencies_with_balance(&[]);
        let init_msg = default_init();
        let env = mock_env();
        instantiate(
            deps.as_mut(),
            env.clone(),
            mock_info("cretor", &[]),
            init_msg,
        )
        .unwrap();

        //first bond
        let info = mock_info(
            "staker1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
        //update balance
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(100),
            }],
        );

        //no index update before update reward index
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.global_index, Decimal256::zero(),);

        //update reward index
        let msg = ExecuteMsg::UpdateRewardIndex {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //index updated after update reward index
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.global_index, Decimal256::one());

        //second bond
        let info = mock_info(
            "staker2",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(200),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //update balance
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(300),
            }],
        );
        //check global index before update
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.global_index, Decimal256::one());

        //update distrubution index
        let msg = ExecuteMsg::UpdateRewardIndex {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //check global index
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(
            config_response.global_index,
            Decimal256::from_ratio(Uint128::new(500), Uint128::new(300))
        );
    }

    #[test]
    pub fn test_recieve_rewards() {
        let mut deps = mock_dependencies_with_balance(&[]);
        let init_msg = default_init();
        let env = mock_env();
        instantiate(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[]),
            init_msg,
        )
        .unwrap();

        //first bond
        let info = mock_info(
            "staker1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //try recieve rewards before any rewards in contract
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::ReceiveReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
        assert_eq!(res.unwrap_err(), ContractError::NoRewards {});

        //update balance
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(100),
            }],
        );

        //update reward index
        let msg = ExecuteMsg::UpdateRewardIndex {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //first bond recieve rewards
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::ReceiveReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        //check first creator rewards
        assert_eq!(
            res.messages.get(0).unwrap().msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "rewards".to_string(),
                    amount: Uint128::new(100),
                }],
            })
        );

        //second bond
        let info = mock_info(
            "staker2",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(200),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //update balance
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(200),
            }],
        );

        //update reward index
        let msg = ExecuteMsg::UpdateRewardIndex {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //first bond recieve rewards again but lower amount
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::ReceiveReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        //check first creator rewards
        assert_eq!(
            res.messages.get(0).unwrap().msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "rewards".to_string(),
                    amount: Uint128::new(33),
                }],
            })
        );
    }

    #[test]
    //test execute update holders rewards
    pub fn test_update_holders_rewards() {
        let mut deps = mock_dependencies_with_balance(&[]);
        let init_msg = default_init();
        let env = mock_env();

        instantiate(
            deps.as_mut(),
            env.clone(),
            mock_info("creator1", &[]),
            init_msg,
        )
        .unwrap();

        //first bond
        let info = mock_info(
            "creator1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );

        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //second bond
        let info = mock_info(
            "creator2",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(200),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //update balance
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(100),
            }],
        );

        //update reward index
        let msg = ExecuteMsg::UpdateRewardIndex {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //update first holders rewards
        let info: MessageInfo = mock_info("creator1", &[]);
        let msg = ExecuteMsg::UpdateHoldersreward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //check first holder rewards
        let res = query(
            deps.as_mut(),
            env.clone(),
            QueryMsg::Holder {
                address: "creator1".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();
        assert_eq!(holder_response.pending_rewards, Uint128::new(33));

        //update second holders rewards
        let info: MessageInfo = mock_info("creator2", &[]);
        let msg = ExecuteMsg::UpdateHoldersreward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //check second holders rewards
        let res = query(
            deps.as_mut(),
            env.clone(),
            QueryMsg::Holder {
                address: "creator2".to_string(),
            },
        )
        .unwrap();

        let holder_response: HolderResponse = from_binary(&res).unwrap();

        assert_eq!(holder_response.pending_rewards, Uint128::new(66));
    }

    //test execute update holders rewards

    //test withdraw
    #[test]
    pub fn test_withdraw() {
        let mut deps = mock_dependencies_with_balance(&[]);
        let init_msg = default_init();
        let env = mock_env();

        instantiate(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[]),
            init_msg,
        )
        .unwrap();

        //first bond
        let info = mock_info(
            "staker1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );

        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //second bond
        let info = mock_info(
            "staker2",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(200),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //update balance
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(100),
            }],
        );

        //withdraw staker1's stake without cap
        let _info: MessageInfo = mock_info("staker1", &[]);
        let _msg = ExecuteMsg::WithdrawStake { amount: None };
        let res = execute(deps.as_mut(), env.clone(), _info.clone(), _msg).unwrap();
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "staker1".to_string(),
                amount: vec![
                    Coin {
                        denom: "rewards".to_string(),
                        amount: Uint128::new(33),
                    },
                    Coin {
                        denom: "staked".to_string(),
                        amount: Uint128::new(100),
                    }
                ],
            }),
        );
        //check second holders rewards
    }
}
