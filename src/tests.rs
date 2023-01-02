#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
    };
    use cosmwasm_std::{
        from_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal256, MessageInfo, Uint128, Uint256,
    };
    use cw_utils::PaymentError;

    use crate::contract::{execute, instantiate, query};
    use crate::msg::{
        ConfigResponse, ExecuteMsg, HolderResponse, InstantiateMsg, QueryMsg, StateResponse,
    };
    use crate::ContractError;

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
            admin: None,
        }
    }

    #[test]
    fn proper_init() {
        let mut deps = mock_dependencies();
        let init_msg = default_init();
        let env = mock_env();
        let info = MessageInfo {
            sender: Addr::unchecked("creator"),
            funds: vec![],
        };
        //instantiate without admin
        let res = instantiate(deps.as_mut(), env.clone(), info, init_msg).unwrap();
        //default response attributes is empty
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        //check if state is correct
        assert_eq!(
            config_response,
            StateResponse {
                global_index: Decimal256::zero(),
                total_staked: Uint128::zero(),
                prev_reward_balance: Uint128::zero(),
            }
        );
        //query config
        let res = query(deps.as_mut(), env.clone(), QueryMsg::Config {}).unwrap();
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
        let res = query(deps.as_mut(), env.clone(), QueryMsg::Config {}).unwrap();
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

        //query contract state for total_staked
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.total_staked, Uint128::new(100),);

        //update balance so we can bond with index update
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(1000000),
            }],
        );

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

        //query staker2's index
        let res = query(
            deps.as_mut(),
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

        //test bond again after withdrawal of user
        let info = mock_info("staker2", &[]);
        let msg = ExecuteMsg::WithdrawStake { amount: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //bond again
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
            deps.as_mut(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker2".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();
        assert_eq!(
            holder_response,
            HolderResponse {
                address: "staker2".to_string(),
                balance: Uint128::new(100),
                index: Decimal256::from_ratio(Uint128::new(1000000), Uint128::new(100)),
                pending_rewards: Uint128::zero(),
                dec_rewards: Decimal256::zero(),
            }
        );
    }

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
        //check prev_reward_balance of state which should be 300 after update
        assert_eq!(config_response.prev_reward_balance, Uint128::new(300));

        //update balance
        deps.querier.update_balance(
            env.contract.address.as_str(),
            vec![Coin {
                denom: "rewards".to_string(),
                amount: Uint128::new(500),
            }],
        );
        //check prev_reward_balance of state which should be 300 before update
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.prev_reward_balance, Uint128::new(300));

        //update distrubution index
        let msg = ExecuteMsg::UpdateRewardIndex {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //check prev_reward_balance of state which should be 500 after update
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let config_response: StateResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.prev_reward_balance, Uint128::new(500));
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
    pub fn test_update_holders_rewards() {
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

        //update_stakers_rewards by random address
        let info = mock_info("random", &[]);
        let msg = ExecuteMsg::UpdateHoldersReward { address: None };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
        assert_eq!(res.unwrap_err(), ContractError::NoBond {});

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

        //update first stakers rewards
        let info: MessageInfo = mock_info("staker1", &[]);
        let msg = ExecuteMsg::UpdateHoldersReward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //check first stakers rewards
        let res = query(
            deps.as_mut(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker1".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();
        assert_eq!(holder_response.pending_rewards, Uint128::new(33));
        assert_eq!(
            holder_response.dec_rewards,
            Decimal256::new(Uint256::from_str("333333333333333300").unwrap())
        );

        //update second stakers rewards
        let info: MessageInfo = mock_info("staker2", &[]);
        let msg = ExecuteMsg::UpdateHoldersReward { address: None };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

        //check second stakers rewards
        let res = query(
            deps.as_mut(),
            env.clone(),
            QueryMsg::Holder {
                address: "staker2".to_string(),
            },
        )
        .unwrap();
        let holder_response: HolderResponse = from_binary(&res).unwrap();

        assert_eq!(holder_response.pending_rewards, Uint128::new(66));
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

        //check state for total staked
        let res = query(deps.as_mut(), env.clone(), QueryMsg::State {}).unwrap();
        let state: StateResponse = from_binary(&res).unwrap();
        assert_eq!(state.total_staked, Uint128::new(200));
    }

    #[test]
    pub fn test_update_config() {
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
        //random can't update config
        let info: MessageInfo = mock_info("random", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            reward_denom: Some("new_reward_denom".to_string()),
            staked_token_denom: Some("new_staked_token_denom".to_string()),
            admin: Some("new_admin".to_string()),
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        //creator can update config
        let info: MessageInfo = mock_info("creator", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            reward_denom: Some("new_reward_denom".to_string()),
            staked_token_denom: Some("new_staked_token_denom".to_string()),
            admin: Some("new_admin".to_string()),
        };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        //check config
        let res = query(deps.as_mut(), env.clone(), QueryMsg::Config {}).unwrap();
        let config_response: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(config_response.admin, "new_admin".to_string());
        assert_eq!(config_response.reward_denom, "new_reward_denom".to_string());
        assert_eq!(
            config_response.staked_token_denom,
            "new_staked_token_denom".to_string()
        );
    }
}
//Tests
// Test the instantiate() function:
// Test that the smart contract is initialized correctly with the correct values for staked_token_denom, reward_denom, and admin.
// Test that the smart contract's global_index and total_staked are set to 0, and that prev_reward_balance is set to 0.
// Test the execute() function:👍
// Test that the BondStake message is handled correctly:
// Test that the user send funds else return error.👍
// Test that the user's funds are in correct denom.👍
// Test that the user's staked tokens are added to the contract's total_staked value.👍
// Test that the user's staker's index is calculated correctly.👍
// Test that the user's staker's index and staked tokens are stored in the contract's state.👍
// Test that the UpdateRewardIndex message is handled correctly:
// Test that the contract's global_index is updated correctly based on the claimed rewards.👍
// Test that the contract's prev_reward_balance is updated correctly.:👍
// Test that the UpdateHoldersReward message is handled correctly:
// Test that the user's rewards are calculated correctly based on their staker's index and staked tokens.:👍
// Test that the user's rewards are added to their balance correctly.:👍
// Test that the WithdrawStake message is handled correctly:
// Test that the user's staked tokens are removed from the contract's total_staked value.:👍
// Test that the user's staker's index and staked tokens are removed from the contract's state.:👍
// Test that the user's rewards are calculated correctly based on their staker's index and staked tokens.:👍
// Test that the user's rewards are transferred to their balance correctly. 👍
// Test that the ReceiveReward message is handled correctly:
// Test that the user's rewards are calculated correctly based on their staker's index and staked tokens.:👍
// Test that the user's rewards are transferred to their balance correctly.:👍
// Test that the UpdateConfig message is handled correctly:
// Test that the staked_token_denom and reward_denom values are updated correctly.:👍
// Test that the admin value is updated correctly.:👍
// Test the query() function:
// Test that the AccruedRewards query returns the correct accrued rewards for the user.
// Test that the Holder query returns the correct staker's index and staked tokens for the user.
// Test that the State query returns the correct values for global_index, total_staked, and prev_reward_balance.
//Errors
// NoBond error: This error should be returned if the total_staked value is 0, indicating that there are no bonded tokens. This error could be returned in the execute_update_reward_index() function if the total_staked value is 0.:👍
// InvalidDenom error: This error should be returned if the input denom is invalid, such as if it is not a recognized token denom. This error could be returned in the execute() function if the input denom is invalid.:👍
// InsufficientFunds error: This error should be returned if the contract does not have sufficient funds to perform the requested action. This error could be returned in the execute_update_holders_rewards() function if the contract does not have enough funds to pay the user their rewards.
// InvalidAdmin error: This error should be returned if the input admin value is invalid, such as if it is not a recognized address. This error could be returned in the execute_update_config() function if the input admin value is invalid.
