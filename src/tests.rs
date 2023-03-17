#[cfg(test)]
mod tests {

    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
    };
    use cosmwasm_std::{
        from_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal256, MessageInfo, StdError, Uint128,
    };
    use cw_utils::PaymentError;

    use crate::contract::{execute, instantiate, query};
    use crate::msg::{
        ConfigResponse, ExecuteMsg, HolderResponse, InstantiateMsg, QueryMsg, StateResponse,
    };
    use crate::{ContractError, state};

    fn default_init() -> InstantiateMsg {
        InstantiateMsg {
            staked_token_denom: "staked".to_string(),
            reward_denom: "rewards".to_string(),
            admin: None,
        }
    }

    #[test]
    fn proper_init() {
        // Instantiate the contract with same denom
        let mut deps = mock_dependencies();
        let mut init_msg = default_init();
        init_msg.staked_token_denom = "rewards".to_string();
        let env = mock_env();
        let info = MessageInfo {
            sender: Addr::unchecked("creator"),
            funds: vec![],
        };
        let res = instantiate(deps.as_mut(), env.clone(), info, init_msg.clone()).unwrap_err();
        assert_eq!(res, ContractError::SameDenom {});

        let mut deps = mock_dependencies();
        let init_msg = default_init();
        let env = mock_env();
        let info = MessageInfo {
            sender: Addr::unchecked("creator"),
            funds: vec![],
        };

        //instantiate without admin
        let res = instantiate(deps.as_mut(), env.clone(), info, init_msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::State {}).unwrap();
        let state: StateResponse = from_binary(&res).unwrap();
        //check if state is correct
        assert_eq!(
            state,
            StateResponse {
                global_index: Decimal256::zero(),
                total_staked: Uint128::zero(),
                total_rewards: Uint128::zero(),
                rewards_claimed: Uint128::zero(),
            }
        );
        //query config
        let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
        let config_response: ConfigResponse = from_binary(&res).unwrap();
        //check if config is correct
        assert_eq!(
            config_response,
            ConfigResponse {
                staked_token_denom: "staked".to_string(),
                reward_denom: "rewards".to_string(),
                admin: "creator".to_string(),
            }
        );
        //instantiate with admin
        let init_msg = InstantiateMsg {
            staked_token_denom: "staked".to_string(),
            reward_denom: "rewards".to_string(),
            admin: Some(Addr::unchecked("admin").to_string()),
        };
        let info = MessageInfo {
            sender: Addr::unchecked("creator"),
            funds: vec![],
        };
        let _res = instantiate(deps.as_mut(), env.clone(), info, init_msg).unwrap();

        //query config
        let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
        let config_response: ConfigResponse = from_binary(&res).unwrap();
        //admin is set to admin
        assert_eq!(config_response.admin, "admin".to_string(),);
    }

    #[test]
    pub fn test_bond() {
        //instantiate
        let mut deps = mock_dependencies();
        let init_msg = default_init();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), env.clone(), info.clone(), init_msg).unwrap();

        //bond with no fund
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::BondStake {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, PaymentError::NoFunds {}.into());

        //bond with wrong denom
        let info = mock_info(
            "random",
            &vec![Coin {
                denom: "wrong".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, PaymentError::MissingDenom("staked".to_string()).into());

        //bond with 0 amount
        let info = mock_info(
            "random",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::zero(),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::Payment(PaymentError::NoFunds {}.into()));

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
            deps.as_ref(),
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

        // Query contract state for total_staked
        let res = query(deps.as_ref(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.total_staked, Uint128::new(100),);

        // Update rewards
        let env = mock_env();
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(1000000),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //second bond
        let info = mock_info(
            "staker2",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // update staker2 rewards
        let env = mock_env();
        let info = mock_info("staker2", &[]);
        let msg = ExecuteMsg::UpdateHolderReward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //query staker2
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker2".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();

        //check if index is correct
        assert_eq!(
            holder_response.index,
            Decimal256::from_ratio(Uint128::new(1000000), Uint128::new(100))
        );
        // check if pending rewards is correct
        assert_eq!(holder_response.pending_rewards, Uint128::new(0));

        // bond again
        let info = mock_info(
            "staker2",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //query staker2
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker2".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();

        //check if index is correct
        assert_eq!(
            holder_response.index,
            Decimal256::from_ratio(Uint128::new(1000000), Uint128::new(100))
        );
        // check if amount is correct
        assert_eq!(holder_response.balance, Uint128::new(200));

        // update rewards
        let env = mock_env();
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(1000000),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // update staker1 rewards
        let env = mock_env();
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::UpdateHolderReward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // update staker2 rewards
        let env = mock_env();
        let info = mock_info("staker2", &[]);
        let msg = ExecuteMsg::UpdateHolderReward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //query staker1
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker1".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();

        //check if index is correct
        assert_eq!(
            holder_response.index,
            Decimal256::from_ratio(Uint128::new(4000000), Uint128::new(300))
        );

        // check if pending rewards is correct
        assert_eq!(holder_response.pending_rewards, Uint128::new(1333333));

        // query staker2
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker2".to_string(),
            },
        );
        let holder_response: HolderResponse = from_binary(&res.unwrap()).unwrap();

        //check if index is correct
        assert_eq!(
            holder_response.index,
            Decimal256::from_ratio(Uint128::new(4000000), Uint128::new(300))
        );

        // check if pending rewards is correct
        assert_eq!(holder_response.pending_rewards, Uint128::new(666666));
    }

    #[test]
    pub fn test_update_reward() {
        // instantiate contract
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

        // update rewards without any bond
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(1000000),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::NoBond {});

        // update reward with multiple denom

        let info = mock_info(
            "creator",
            &vec![
                Coin {
                    denom: "rewards".to_string(),
                    amount: Uint128::new(1000000),
                },
                Coin {
                    denom: "rewards2".to_string(),
                    amount: Uint128::new(1000000),
                },
            ],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::Payment(PaymentError::MultipleDenoms {}));

        // update reward with wrong denom
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "wrong".to_string(),
                amount: Uint128::new(1000000),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::Payment(PaymentError::MissingDenom("rewards".to_string()))
        );

        // update reward with 0 amount
        let info= mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(0),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::Payment(PaymentError::NoFunds {}));

        // bond staker1
        let info = mock_info(
            "staker1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // update reward
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(1000000),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // query state
        let res = query(deps.as_ref(), env.clone(), QueryMsg::State {}).unwrap();
        let state_response: StateResponse = from_binary(&res).unwrap();

        // check if reward index is correct
        assert_eq!(
            state_response.global_index,
            Decimal256::from_ratio(Uint128::new(1000000), Uint128::new(100))
        );

        // check if reward pool is correct
        assert_eq!(state_response.total_rewards, Uint128::new(1000000));
    }

    #[test]
    pub fn test_update_holder_rewards() {
        // instantiate contract
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

        // update staker 1 rewards without bonding
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::UpdateHolderReward {
            address: Some("staker1".to_string()),
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::Std(
                (StdError::NotFound {
                    kind: ("cw_share::state::Holder").to_string()
                })
            )
        );
        // bond staker1
        let info = mock_info(
            "staker1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // query staker 1
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker1".to_string(),
            },
        )
        .unwrap();

        let holder_response: HolderResponse = from_binary(&res).unwrap();

        // check if holder data is correct
        assert_eq!(holder_response.pending_rewards, Uint128::new(0));
        assert_eq!(holder_response.index, Decimal256::zero());
        assert_eq!(holder_response.balance, Uint128::new(100));

        // update reward
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(1000000),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // update staker 1 rewards
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::UpdateHolderReward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // query staker 1
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker1".to_string(),
            },
        )
        .unwrap();

        let holder_response: HolderResponse = from_binary(&res).unwrap();

        // check if pending rewards is correct
        assert_eq!(holder_response.pending_rewards, Uint128::new(1000000));

        // check if reward index is correct
        assert_eq!(
            holder_response.index,
            Decimal256::from_ratio(Uint128::new(1000000), Uint128::new(100))
        );
    }

    #[test]
    pub fn test_receive_rewards() {
        // instantiate contract
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

        // try to receive rewards without bonding
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::ReceiveReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::Std(
                (StdError::NotFound {
                    kind: ("cw_share::state::Holder").to_string()
                })
            )
        );

        // bond staker1
        let info = mock_info(
            "staker1",
            &vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::BondStake {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // try to receive rewards without any reward
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::ReceiveReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::NoRewards {});

        // update reward
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(1000000),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // receive rewards
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::ReceiveReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "staker1".to_string(),
                amount: vec![Coin {
                    denom: "rewards".to_string(),
                    amount: Uint128::new(1000000),
                }],
            })
        );

        // query staker 1
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker1".to_string(),
            },
        )
        .unwrap();

        let holder_response: HolderResponse = from_binary(&res).unwrap();
        // check if pending rewards is correct
        assert_eq!(holder_response.pending_rewards, Uint128::new(0));
        // check if reward index is correct
        assert_eq!(
            holder_response.index,
            Decimal256::from_ratio(Uint128::new(1000000), Uint128::new(100))
        );
        // query state
        let res = query(deps.as_ref(), env.clone(), QueryMsg::State {}).unwrap();
        let state_response: StateResponse = from_binary(&res).unwrap();

        assert_eq!(state_response.total_rewards, Uint128::new(1000000));
        assert_eq!(state_response.rewards_claimed, Uint128::new(1000000));

        // try to receive rewards again
        let info = mock_info("staker1", &[]);
        let msg = ExecuteMsg::ReceiveReward {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::NoRewards {});
    }

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

        //update reward
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //withdraw staker1's stake without cap
        let _info: MessageInfo = mock_info("staker1", &[]);
        let _msg = ExecuteMsg::WithdrawStake { amount: None };
        let res = execute(deps.as_mut(), env.clone(), _info.clone(), _msg).unwrap();
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "staker1".to_string(),
                amount: vec![Coin {
                    denom: "rewards".to_string(),
                    amount: Uint128::new(33),
                }],
            }),
        );
        assert_eq!(
            res.messages[1].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "staker1".to_string(),
                amount: vec![Coin {
                    denom: "staked".to_string(),
                    amount: Uint128::new(100),
                }],
            }),
        );

        //check state for total staked
        let res = query(deps.as_ref(), env.clone(), QueryMsg::State {}).unwrap();
        let state: StateResponse = from_binary(&res).unwrap();
        assert_eq!(state.total_staked, Uint128::new(200));

        //withdraw staker2's stake with cap
        let _info: MessageInfo = mock_info("staker2", &[]);
        let _msg = ExecuteMsg::WithdrawStake {
            amount: Some(Uint128::new(100)),
        };
        let res = execute(deps.as_mut(), env.clone(), _info.clone(), _msg).unwrap();
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "staker2".to_string(),
                amount: vec![Coin {
                    denom: "rewards".to_string(),
                    amount: Uint128::new(66),
                }],
            }),
        );
        assert_eq!(res.messages[1].msg, CosmosMsg::Bank(BankMsg::Send {
            to_address: "staker2".to_string(),
            amount: vec![Coin {
                denom: "staked".to_string(),
                amount: Uint128::new(100),
            }],
        }));

        //check state for total staked
        let res = query(deps.as_ref(), env.clone(), QueryMsg::State {}).unwrap();
        let state: StateResponse = from_binary(&res).unwrap();
        assert_eq!(state.total_staked, Uint128::new(100));


    }

    #[test]
    pub fn test_update_admin() {
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
        //random can't update admin
        let info = mock_info("random", &[]);
        let msg = ExecuteMsg::UpdateAdmin {
            address: "new_admin".to_string(),
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // admin can update admin
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::UpdateAdmin {
            address: "new_admin".to_string(),
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(res.attributes[1].value, "new_admin".to_string());
        
    }

    #[test]
    pub fn test_admin_withdraw_all(){
        // init
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

        // update reward
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(100),
            }],
        );
        let msg = ExecuteMsg::UpdateReward {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // now contract has 100 rewards and 100 staked
        // random can not withdraw all
        let info = mock_info("random", &[]);
        let msg = ExecuteMsg::AdminWithdrawAll {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // admin can withdraw all
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::AdminWithdrawAll {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: vec![Coin {
                    denom: "rewards".to_string(),
                    amount: Uint128::new(100),
                }],
            }),
        );
        assert_eq!(
            res.messages[1].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: vec![Coin {
                    denom: "staked".to_string(),
                    amount: Uint128::new(100),
                }],
            }),
        );





    }

}   