#![allow(dead_code)]

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::{instruction::Instruction, system_program};
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::associated_token::{
    self, get_associated_token_address, spl_associated_token_account,
};
use anchor_spl::token;
use q3_26_core_staking::{CONFIG_SEED, REWARDS_MINT_SEED, UPDATE_AUTHORITY_SEED};

pub fn config_pda(collection: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[CONFIG_SEED, collection.as_ref()],
        &q3_26_core_staking::id(),
    )
    .0
}

pub fn update_authority_pda(collection: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[UPDATE_AUTHORITY_SEED, collection.as_ref()],
        &q3_26_core_staking::id(),
    )
    .0
}

pub fn rewards_mint_pda(collection: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[REWARDS_MINT_SEED, config_pda(collection).as_ref()],
        &q3_26_core_staking::id(),
    )
    .0
}

pub fn rewards_ata(owner: &Pubkey, collection: &Pubkey) -> Pubkey {
    get_associated_token_address(owner, &rewards_mint_pda(collection))
}

/// The program does not create the rewards ATA: the owner opens it before claiming.
pub fn create_rewards_ata_instruction(owner: &Pubkey, collection: &Pubkey) -> Instruction {
    spl_associated_token_account::instruction::create_associated_token_account(
        owner,
        owner,
        &rewards_mint_pda(collection),
        &token::ID,
    )
}

pub fn create_collection_instruction(payer: &Pubkey, collection: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_core_staking::id(),
        &q3_26_core_staking::instruction::CreateCollection {
            name: "Staking Collection".to_string(),
            uri: "https://example.com/collection.json".to_string(),
        }
        .data(),
        q3_26_core_staking::accounts::CreateCollection {
            payer: *payer,
            collection: *collection,
            update_authority: update_authority_pda(collection),
            system_program: system_program::ID,
            mpl_core_program: mpl_core::ID,
        }
        .to_account_metas(None),
    )
}

pub fn initialize_instruction(
    admin: &Pubkey,
    collection: &Pubkey,
    rewards_bps: u16,
    freeze_period: u16,
) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_core_staking::id(),
        &q3_26_core_staking::instruction::Initialize {
            rewards_bps,
            freeze_period,
        }
        .data(),
        q3_26_core_staking::accounts::Initialize {
            admin: *admin,
            config: config_pda(collection),
            collection: *collection,
            update_authority: update_authority_pda(collection),
            rewards_mint: rewards_mint_pda(collection),
            system_program: system_program::ID,
            token_program: token::ID,
        }
        .to_account_metas(None),
    )
}

pub fn mint_asset_instruction(user: &Pubkey, asset: &Pubkey, collection: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_core_staking::id(),
        &q3_26_core_staking::instruction::MintAsset {
            name: "Staking Asset".to_string(),
            uri: "https://example.com/asset.json".to_string(),
        }
        .data(),
        q3_26_core_staking::accounts::MintAsset {
            user: *user,
            asset: *asset,
            collection: *collection,
            update_authority: update_authority_pda(collection),
            system_program: system_program::ID,
            mpl_core_program: mpl_core::ID,
        }
        .to_account_metas(None),
    )
}

pub fn stake_instruction(owner: &Pubkey, asset: &Pubkey, collection: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_core_staking::id(),
        &q3_26_core_staking::instruction::Stake {}.data(),
        q3_26_core_staking::accounts::Stake {
            owner: *owner,
            config: config_pda(collection),
            asset: *asset,
            collection: *collection,
            update_authority: update_authority_pda(collection),
            system_program: system_program::ID,
            mpl_core_program: mpl_core::ID,
        }
        .to_account_metas(None),
    )
}

/// `unstake`, `claim_rewards` and `burn_staked_nft` take the same accounts.
macro_rules! rewards_instruction {
    ($name:ident, $owner:expr, $asset:expr, $collection:expr) => {
        Instruction::new_with_bytes(
            q3_26_core_staking::id(),
            &q3_26_core_staking::instruction::$name {}.data(),
            q3_26_core_staking::accounts::$name {
                owner: *$owner,
                config: config_pda($collection),
                asset: *$asset,
                collection: *$collection,
                update_authority: update_authority_pda($collection),
                rewards_mint: rewards_mint_pda($collection),
                user_rewards_ata: rewards_ata($owner, $collection),
                token_program: token::ID,
                associated_token_program: associated_token::ID,
                system_program: system_program::ID,
                mpl_core_program: mpl_core::ID,
            }
            .to_account_metas(None),
        )
    };
}

pub fn unstake_instruction(owner: &Pubkey, asset: &Pubkey, collection: &Pubkey) -> Instruction {
    rewards_instruction!(Unstake, owner, asset, collection)
}

pub fn claim_rewards_instruction(
    owner: &Pubkey,
    asset: &Pubkey,
    collection: &Pubkey,
) -> Instruction {
    rewards_instruction!(ClaimRewards, owner, asset, collection)
}

pub fn burn_staked_nft_instruction(
    owner: &Pubkey,
    asset: &Pubkey,
    collection: &Pubkey,
) -> Instruction {
    rewards_instruction!(BurnStakedNft, owner, asset, collection)
}
