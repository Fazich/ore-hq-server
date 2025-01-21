use std::{str::FromStr, time::{SystemTime, UNIX_EPOCH}};

use bytemuck::{Pod, Zeroable};
use drillx::Solution;
use ore_api::{
    consts::{BUS_ADDRESSES, CONFIG_ADDRESS, MINT_ADDRESS, PROOF, TOKEN_DECIMALS},
    state::{Config, Proof},
    ID as ORE_ID,
};
use ore_boost_api::state::{boost_pda, stake_pda};
use ore_miner_delegation::{instruction, pda::managed_proof_pda, state::{DelegatedBoost, DelegatedBoostV2, DelegatedStake}, utils::AccountDeserialize};
use ore_utils::event;
pub use steel::AccountDeserialize as _;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{account::ReadableAccount, instruction::Instruction, pubkey::Pubkey};
use spl_associated_token_account::get_associated_token_address;

pub const ORE_TOKEN_DECIMALS: u8 = TOKEN_DECIMALS;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MineEventWithBoosts {
    pub balance: u64,
    pub difficulty: u64,
    pub last_hash_at: i64,
    pub timing: i64,
    pub reward: u64,
    pub boost_1: u64,
    pub boost_2: u64,
    pub boost_3: u64,
}

event!(MineEventWithBoosts);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MineEventWithGlobalBoosts {
    pub balance: u64,
    pub difficulty: u64,
    pub last_hash_at: i64,
    pub timing: i64,
    pub net_reward: u64,
    pub net_base_reward: u64,
    pub net_miner_boost_reward: u64,
    pub net_staker_boost_reward: u64,
}

event!(MineEventWithGlobalBoosts);

pub fn get_auth_ix(signer: Pubkey) -> Instruction {
    let proof = get_proof_pda(signer);

    ore_api::prelude::auth(proof)
}

pub fn get_mine_ix(signer: Pubkey, solution: Solution, bus: usize) -> Instruction {
    instruction::mine(signer, BUS_ADDRESSES[bus], solution)
}

pub fn get_mine_ix_with_boosts(signer: Pubkey, solution: Solution, bus: usize, boost_mints: Vec<Pubkey>) -> Instruction {
    let managed_proof_account = managed_proof_pda(signer);
    let mut boosts = Vec::new();

    // for boost_mint in boost_mints {
    //     let boost_account = boost_pda(boost_mint);
    //     let boost_stake = stake_pda(managed_proof_account.0, boost_account.0);
    //     boosts.push(boost_account.0);
    //     boosts.push(boost_stake.0);
    // }

    instruction::mine_with_boost(signer, BUS_ADDRESSES[bus], solution, boosts)
}

pub fn get_register_ix(signer: Pubkey) -> Instruction {
    instruction::open_managed_proof(signer)
}

pub fn get_reset_ix(signer: Pubkey) -> Instruction {
    ore_api::prelude::reset(signer)
}

pub fn get_claim_ix(signer: Pubkey, beneficiary: Pubkey, claim_amount: u64) -> Instruction {
    instruction::undelegate_stake(signer, signer, beneficiary, claim_amount)
}

pub fn get_stake_ix(signer: Pubkey, sender: Pubkey, stake_amount: u64) -> Instruction {
    instruction::delegate_stake(sender, signer, stake_amount)
}

pub fn get_ore_mint() -> Pubkey {
    MINT_ADDRESS
}

pub fn get_managed_proof_token_ata(miner: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &ore_miner_delegation::id(),
    );

    get_associated_token_address(&managed_proof.0, &ore_api::consts::MINT_ADDRESS)
}

pub fn get_proof_pda(miner: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &ore_miner_delegation::id(),
    );

    proof_pubkey(managed_proof.0)
}

pub async fn get_delegated_stake_account(
    client: &RpcClient,
    staker: Pubkey,
    miner: Pubkey,
) -> Result<ore_miner_delegation::state::DelegatedStake, String> {
    let data = client
        .get_account_data(&get_delegated_stake_pda(staker, miner))
        .await;
    match data {
        Ok(data) => {
            let delegated_stake = DelegatedStake::try_from_bytes(&data);
            if let Ok(delegated_stake) = delegated_stake {
                return Ok(*delegated_stake);
            } else {
                return Err("Failed to parse delegated stake account".to_string());
            }
        }
        Err(_) => return Err("Failed to get delegated stake account".to_string()),
    }
}

pub async fn get_delegated_boost_account(
    client: &RpcClient,
    staker: Pubkey,
    miner: Pubkey,
    mint: Pubkey,
) -> Result<ore_miner_delegation::state::DelegatedBoost, String> {
    let data = client
        .get_account_data(&get_delegated_boost_pda(staker, miner, mint))
        .await;
    match data {
        Ok(data) => {
            let delegated_boost = DelegatedBoost::try_from_bytes(&data);
            if let Ok(delegated_boost) = delegated_boost {
                return Ok(*delegated_boost);
            } else {
                return Err("Failed to parse delegated boost account".to_string());
            }
        }
        Err(_) => return Err("Failed to get delegated boost account".to_string()),
    }
}

pub async fn get_delegated_boost_account_v2(
    client: &RpcClient,
    staker: Pubkey,
    miner: Pubkey,
    mint: Pubkey,
) -> Result<ore_miner_delegation::state::DelegatedBoostV2, String> {
    let data = client
        .get_account_data(&get_delegated_boost_v2_pda(staker, miner, mint))
        .await;
    match data {
        Ok(data) => {
            let delegated_boost = DelegatedBoostV2::try_from_bytes(&data);
            if let Ok(delegated_boost) = delegated_boost {
                return Ok(*delegated_boost);
            } else {
                return Err("Failed to parse delegated boost v2 account".to_string());
            }
        }
        Err(_) => return Err("Failed to get delegated boost v2 account".to_string()),
    }
}

pub fn get_delegated_stake_pda(staker: Pubkey, miner: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &ore_miner_delegation::id(),
    );

    Pubkey::find_program_address(
        &[
            b"delegated-stake",
            staker.as_ref(),
            managed_proof.0.as_ref(),
        ],
        &ore_miner_delegation::id(),
    )
    .0
}

pub fn get_delegated_boost_pda(staker: Pubkey, miner: Pubkey, mint: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &ore_miner_delegation::id(),
    );

    Pubkey::find_program_address(
        &[
            ore_miner_delegation::consts::DELEGATED_BOOST,
            staker.as_ref(),
            mint.as_ref(),
            managed_proof.0.as_ref(),
        ],
        &ore_miner_delegation::id(),
    )
    .0
}

pub fn get_delegated_boost_v2_pda(staker: Pubkey, miner: Pubkey, mint: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &ore_miner_delegation::id(),
    );

    Pubkey::find_program_address(
        &[
            ore_miner_delegation::consts::DELEGATED_BOOST_V2,
            staker.as_ref(),
            mint.as_ref(),
            managed_proof.0.as_ref(),
        ],
        &ore_miner_delegation::id(),
    )
    .0
}


pub async fn get_config(client: &RpcClient) -> Result<ore_api::state::Config, String> {
    let data = client.get_account_data(&CONFIG_ADDRESS).await;
    match data {
        Ok(data) => {
            let config = Config::try_from_bytes(&data);
            if let Ok(config) = config {
                return Ok(*config);
            } else {
                return Err("Failed to parse config account".to_string());
            }
        }
        Err(_) => return Err("Failed to get config account".to_string()),
    }
}

pub async fn get_proof_and_config_with_busses(
    client: &RpcClient,
    authority: Pubkey,
) -> (
    Result<Proof, ()>,
    Result<ore_api::state::Config, ()>,
    Result<Vec<Result<ore_api::state::Bus, ()>>, ()>,
) {
    let account_pubkeys = vec![
        get_proof_pda(authority),
        CONFIG_ADDRESS,
        BUS_ADDRESSES[0],
        BUS_ADDRESSES[1],
        BUS_ADDRESSES[2],
        BUS_ADDRESSES[3],
        BUS_ADDRESSES[4],
        BUS_ADDRESSES[5],
        BUS_ADDRESSES[6],
        BUS_ADDRESSES[7],
    ];
    let datas = client.get_multiple_accounts(&account_pubkeys).await;
    if let Ok(datas) = datas {
        let proof = if let Some(data) = &datas[0] {
            Ok(*Proof::try_from_bytes(data.data()).expect("Failed to parse treasury account"))
        } else {
            Err(())
        };

        let treasury_config = if let Some(data) = &datas[1] {
            Ok(*ore_api::state::Config::try_from_bytes(data.data())
                .expect("Failed to parse config account"))
        } else {
            Err(())
        };
        let bus_1 = if let Some(data) = &datas[2] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus1 account"))
        } else {
            Err(())
        };
        let bus_2 = if let Some(data) = &datas[3] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus2 account"))
        } else {
            Err(())
        };
        let bus_3 = if let Some(data) = &datas[4] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus3 account"))
        } else {
            Err(())
        };
        let bus_4 = if let Some(data) = &datas[5] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus4 account"))
        } else {
            Err(())
        };
        let bus_5 = if let Some(data) = &datas[6] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus5 account"))
        } else {
            Err(())
        };
        let bus_6 = if let Some(data) = &datas[7] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus6 account"))
        } else {
            Err(())
        };
        let bus_7 = if let Some(data) = &datas[8] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus7 account"))
        } else {
            Err(())
        };
        let bus_8 = if let Some(data) = &datas[9] {
            Ok(*ore_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus1 account"))
        } else {
            Err(())
        };

        (
            proof,
            treasury_config,
            Ok(vec![bus_1, bus_2, bus_3, bus_4, bus_5, bus_6, bus_7, bus_8]),
        )
    } else {
        (Err(()), Err(()), Err(()))
    }
}

pub async fn get_original_proof(client: &RpcClient, authority: Pubkey) -> Result<Proof, String> {
    let proof_address = proof_pubkey(authority);
    let data = client.get_account_data(&proof_address).await;
    match data {
        Ok(data) => {
            let proof = Proof::try_from_bytes(&data);
            if let Ok(proof) = proof {
                return Ok(*proof);
            } else {
                return Err("Failed to parse proof account".to_string());
            }
        }
        Err(_) => return Err("Failed to get proof account".to_string()),
    }
}

pub async fn get_pool_boost_stake(rpc_client: &RpcClient, authority: Pubkey) -> Vec<ore_boost_api::state::Stake> {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", authority.as_ref()],
        &ore_miner_delegation::id(),
    );

    let boost_mints = vec![
        Pubkey::from_str("oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp").unwrap(),
        Pubkey::from_str("DrSS5RM7zUd9qjUEdDaf31vnDUSbCrMto6mjqTrHFifN").unwrap(),
        Pubkey::from_str("meUwDp23AaxhiNKaQCyJ2EAF2T4oe1gSkEkGXSRVdZb").unwrap()
    ];

    // Get pools boost stake accounts
    let mut boost_stake_acct_pdas = vec![];

    for boost_mint in boost_mints {
        let boost_account_pda = boost_pda(boost_mint);
        let boost_stake_pda = stake_pda(managed_proof.0, boost_account_pda.0);
        boost_stake_acct_pdas.push(boost_stake_pda.0);
    }

    let mut stake_acct = vec![];
    if let Ok(accounts) = rpc_client.get_multiple_accounts(&boost_stake_acct_pdas).await {
        for account in accounts {
        }
    } else {
        tracing::error!(target: "server_log", "Failed to get pool boost accounts.")
    }

    return stake_acct;
}

pub async fn get_proof(client: &RpcClient, authority: Pubkey) -> Result<Proof, String> {
    let proof_address = get_proof_pda(authority);
    let data = client.get_account_data(&proof_address).await;
    match data {
        Ok(data) => {
            let proof = Proof::try_from_bytes(&data);
            if let Ok(proof) = proof {
                return Ok(*proof);
            } else {
                return Err("Failed to parse proof account".to_string());
            }
        }
        Err(_) => return Err("Failed to get proof account".to_string()),
    }
}

pub fn proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ORE_ID).0
}

pub fn get_cutoff(proof: Proof, buffer_time: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get time")
        .as_secs() as i64;
    proof
        .last_hash_at
        .saturating_add(60)
        .saturating_sub(buffer_time as i64)
        .saturating_sub(now)
}
