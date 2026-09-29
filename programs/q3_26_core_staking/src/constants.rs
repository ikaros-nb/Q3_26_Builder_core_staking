use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const UPDATE_AUTHORITY_SEED: &[u8] = b"update_authority";

#[constant]
pub const REWARDS_MINT_SEED: &[u8] = b"rewards_mint";

pub const SECONDS_PER_DAY: i64 = 86400;

// One-time bonus (in whole reward tokens) minted on top of the staking rewards when burning a staked NFT
#[constant]
pub const BURN_BONUS: u64 = 1_000;
