# Custom Reward Contract

This is a custom reward contract built using the CosmWasm smart contract development framework. The contract allows users to stake tokens and receive rewards based on their staked amount. 

## Table of Contents

- [Overview](#overview)
- [Getting Started](#getting-started)
  - [Instantiate](#instantiate)
  - [Execute](#execute)
    - [Update Reward](#update-reward)
    - [Bond Stake](#bond-stake)
    - [Update Holder Rewards](#update-holder-rewards)
    - [Withdraw Stake](#withdraw-stake)
    - [Receive Reward](#receive-reward)
    - [Admin Withdraw All](#admin-withdraw-all)
    - [Update Admin](#update-admin)
- [Queries](#queries)
  - [State](#state)
  - [Config](#config)
  - [Accrued Rewards](#accrued-rewards)
  - [Holder](#holder)
  - [Holders List](#holders-list)
- [Migrate](#migrate)

## Overview

This contract enables users to stake tokens and receive rewards based on the global reward index. Users can bond (stake) tokens, withdraw their stakes, and claim rewards. The rewards are calculated based on the staked amount, the global reward index, and the pending rewards. The contract also supports administrative actions such as updating the admin address and performing a full withdrawal of all tokens.
I have developed this contract to facilitate revenue distribution within Decentralized Autonomous Organizations (DAOs). Utilizing this contract, a DAO's multisig authority can generate custom tokens and allocate them among its members. This tokenized distribution can then be staked by members, enabling the equitable allocation of generated revenues.

## Getting Started

### Instantiate

In the `instantiate` function, the contract is initialized with the following parameters:
- `admin`: The admin address, which can perform administrative actions.
- `staked_token_denom`: The denomination of the staked token.
- `reward_denom`: The denomination of the reward token.

### Execute

The contract supports various execution messages (`ExecuteMsg`) that users can send to perform actions:

#### Update Reward

- `execute_update_reward`: Updates the reward by increasing the global index and total rewards based on the provided amount.

#### Bond Stake

- `execute_bond`: Allows users to stake tokens, increasing their balance and the total staked amount.

#### Update Holder Rewards

- `execute_update_holder_rewards`: Updates the rewards for a specific holder based on the global index and their staked balance.

#### Withdraw Stake

- `execute_withdraw`: Allows users to withdraw their staked tokens, along with claiming any pending rewards. Unlike other bonding contracts, there is no unbonding period holder can withdraw staked tokens instantly.

#### Receive Reward

- `execute_receive_reward`: Allows users to claim pending rewards.

#### Admin Withdraw All

- `execute_admin_withdraw_all`: Allows the admin to withdraw all tokens from the contract.

#### Update Admin

- `execute_update_admin`: Allows the admin to update the contract's admin address.

## Queries

The contract supports several queries (`QueryMsg`) that provide information about the contract's state:

### State

- `query_state`: Retrieves the current state of the contract, including the total staked amount, global index, total rewards, and rewards claimed.

### Config

- `query_config`: Retrieves the contract's configuration, including staked token denomination, reward token denomination, and admin address.

### Accrued Rewards

- `query_accrued_rewards`: Retrieves the pending rewards for a specific address.

### Holder

- `query_holder`: Retrieves information about a specific holder, including their address, balance, index, pending rewards, and decimal rewards.

### Holders List

- `query_holders`: Retrieves a list of holders with optional pagination.

## Migrate

The `migrate` function is provided for potential future contract migrations, although it currently returns a default response.
