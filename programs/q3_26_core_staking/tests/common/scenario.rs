#![allow(dead_code)]

use super::*;
use q3_26_core_staking::SECONDS_PER_DAY;

/// 100%: one whole reward token per staked day
pub const REWARDS_BPS: u16 = 10_000;
/// In days
pub const FREEZE_PERIOD: u16 = 2;
pub const DAY: i64 = SECONDS_PER_DAY;
/// One whole reward token: the rewards mint has 6 decimals
pub const TOKEN: u64 = 1_000_000;

/// Whole days as seconds, for `env.warp`.
pub fn days(days: u16) -> i64 {
    days as i64 * DAY
}

/// Builds one program instruction from (owner, asset, collection).
pub type Build = fn(&Pubkey, &Pubkey, &Pubkey) -> Instruction;

/// A collection under the program authority with staking initialized, and one
/// asset minted to `user`, who already holds a rewards ATA.
pub struct Scenario {
    pub env: Env,
    pub user: Keypair,
    pub collection: Pubkey,
    pub asset: Pubkey,
}

impl Scenario {
    pub fn new() -> Self {
        let mut env = Env::new();
        let admin = env.payer.pubkey();
        let user = env.funded_keypair();
        let collection = Keypair::new();
        let asset = Keypair::new();

        succeed(env.send(
            &[create_collection_instruction(&admin, &collection.pubkey())],
            &[&collection],
        ));
        succeed(env.send(
            &[initialize_instruction(
                &admin,
                &collection.pubkey(),
                REWARDS_BPS,
                FREEZE_PERIOD,
            )],
            &[],
        ));
        succeed(env.send(
            &[mint_asset_instruction(
                &user.pubkey(),
                &asset.pubkey(),
                &collection.pubkey(),
            )],
            &[&user, &asset],
        ));
        succeed(env.send(
            &[create_rewards_ata_instruction(
                &user.pubkey(),
                &collection.pubkey(),
            )],
            &[&user],
        ));

        Self {
            env,
            user,
            collection: collection.pubkey(),
            asset: asset.pubkey(),
        }
    }

    /// Same, with the asset staked at the current Clock.
    pub fn staked() -> Self {
        let mut scenario = Self::new();
        scenario.stake();
        scenario
    }

    /// Sends `build` for the user's asset, signed by the user.
    pub fn try_as_user(&mut self, build: Build) -> TransactionResult {
        let instruction = build(&self.user.pubkey(), &self.asset, &self.collection);
        self.env.send(&[instruction], &[&self.user])
    }

    /// Sends `build` for the user's asset, signed by someone else.
    /// The stranger holds a rewards ATA: Anchor loads every account before `has_one`,
    /// so a missing ATA would fail first and hide the owner check.
    pub fn try_as_stranger(&mut self, build: Build) -> TransactionResult {
        let stranger = self.env.funded_keypair();
        succeed(self.env.send(
            &[create_rewards_ata_instruction(
                &stranger.pubkey(),
                &self.collection,
            )],
            &[&stranger],
        ));
        let instruction = build(&stranger.pubkey(), &self.asset, &self.collection);
        self.env.send(&[instruction], &[&stranger])
    }

    pub fn try_stake(&mut self) -> TransactionResult {
        self.try_as_user(stake_instruction)
    }

    pub fn try_unstake(&mut self) -> TransactionResult {
        self.try_as_user(unstake_instruction)
    }

    pub fn try_claim(&mut self) -> TransactionResult {
        self.try_as_user(claim_rewards_instruction)
    }

    pub fn try_burn(&mut self) -> TransactionResult {
        self.try_as_user(burn_staked_nft_instruction)
    }

    pub fn stake(&mut self) {
        succeed(self.try_stake());
    }

    pub fn unstake(&mut self) {
        succeed(self.try_unstake());
    }

    pub fn claim(&mut self) {
        succeed(self.try_claim());
    }

    pub fn burn(&mut self) {
        succeed(self.try_burn());
    }

    /// A second asset minted to the user, not staked.
    pub fn mint_another_asset(&mut self) -> Pubkey {
        let asset = Keypair::new();
        succeed(self.env.send(
            &[mint_asset_instruction(
                &self.user.pubkey(),
                &asset.pubkey(),
                &self.collection,
            )],
            &[&self.user, &asset],
        ));
        asset.pubkey()
    }

    pub fn rewards(&self) -> u64 {
        self.env.rewards(&self.user.pubkey(), &self.collection)
    }

    pub fn attribute(&self, key: &str) -> Option<String> {
        self.env.attribute(&self.asset, key)
    }

    /// The "total_staked" counter of the collection, `None` before the first stake.
    pub fn total_staked(&self) -> Option<u64> {
        self.env
            .collection_attribute(&self.collection, "total_staked")
            .map(|value| value.parse().unwrap())
    }

    pub fn is_frozen(&self) -> bool {
        self.env
            .asset(&self.asset)
            .and_then(|asset| asset.plugin_list.freeze_delegate)
            .is_some_and(|plugin| plugin.freeze_delegate.frozen)
    }
}
