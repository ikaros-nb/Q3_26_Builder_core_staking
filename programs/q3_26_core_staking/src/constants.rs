use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const UPDATE_AUTHORITY_SEED: &[u8] = b"update_authority";

#[constant]
pub const REWARDS_MINT_SEED: &[u8] = b"rewards_mint";

pub const SECONDS_PER_DAY: i64 = 86400;
