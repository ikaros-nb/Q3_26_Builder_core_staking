use crate::{constants::*, error::CoreStakingError, state::Config, utils::update_total_staked};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{mint_to_checked, Mint, MintToChecked, TokenAccount, TokenInterface},
};
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::{BurnV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{Attributes, FreezeDelegate, Plugin, PluginType, UpdateAuthority},
    ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct BurnStakedNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED, collection.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        has_one = owner @ CoreStakingError::InvalidOwner,
        constraint = asset.update_authority == UpdateAuthority::Collection(collection.key()) @ CoreStakingError::InvalidUpdateAuthority,
    )]
    pub asset: Account<'info, BaseAssetV1>,
    #[account(
        mut,
        has_one = update_authority @ CoreStakingError::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1>,
    /// CHECK: This account is not initialized and is being used for signing purposes only, we verify that derives from the correct seeds
    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [REWARDS_MINT_SEED, config.key().as_ref()],
        bump = config.rewards_bump,
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = rewards_mint,
        associated_token::authority = owner,
    )]
    pub user_rewards_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    /// CHECK: This is the ID of the MPL Core Program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
}

impl<'info> BurnStakedNft<'info> {
    pub fn burn_staked_nft(&self, bumps: &BurnStakedNftBumps) -> Result<()> {
        // We start by fetching the existing attributes; if the plugin is missing the asset is not staked
        let attributes: Attributes = fetch_plugin::<BaseAssetV1, Attributes>(
            &self.asset.to_account_info(),
            PluginType::Attributes,
        )
        .map(|(_, attrs, _)| attrs)
        .map_err(|_| CoreStakingError::AssetNotStaked)?;

        let mut staked_found = false;
        let mut staked_timestamp: Option<i64> = None;

        // No need to rebuild the attributes list: the asset and its plugins are gone after the burn
        for attribute in &attributes.attribute_list {
            if attribute.key == "staked" {
                require!(attribute.value == "true", CoreStakingError::AssetNotStaked);
                staked_found = true;
            } else if attribute.key == "staked_at" {
                staked_timestamp = Some(
                    attribute
                        .value
                        .parse::<i64>()
                        .map_err(|_| CoreStakingError::InvalidTimestamp)?,
                );
            }
        }

        // Both staking attributes must be present, otherwise the asset is not staked by this program
        require!(staked_found, CoreStakingError::AssetNotStaked);
        let staked_timestamp = staked_timestamp.ok_or(CoreStakingError::AssetNotStaked)?;

        // Elapsed time in whole days since the last checkpoint (stake or previous claim)
        let elapsed_days = Clock::get()?
            .unix_timestamp
            .checked_sub(staked_timestamp)
            .ok_or(CoreStakingError::InvalidTimestamp)?
            .checked_div(SECONDS_PER_DAY)
            .ok_or(CoreStakingError::InvalidTimestamp)?;

        // Same lock as unstake, otherwise burning would be a way around the freeze period
        require!(
            elapsed_days >= self.config.freeze_period as i64,
            CoreStakingError::FreezePeriodNotElapsed
        );

        // Calculate the amount before burning: the asset account is closed afterwards
        let decimals_factor = 10u64.pow(self.rewards_mint.decimals as u32);
        let staking_rewards = (elapsed_days as u64)
            .checked_mul(self.config.rewards_bps as u64)
            .ok_or(CoreStakingError::InvalidRewardsBps)?
            .checked_mul(decimals_factor)
            .ok_or(CoreStakingError::InvalidRewardsBps)?
            .checked_div(10000u64)
            .ok_or(CoreStakingError::InvalidRewardsBps)?;
        let burn_bonus = BURN_BONUS
            .checked_mul(decimals_factor)
            .ok_or(CoreStakingError::InvalidRewardsBps)?;
        let amount = staking_rewards
            .checked_add(burn_bonus)
            .ok_or(CoreStakingError::InvalidRewardsBps)?;

        // Prepare signing seeds for the update authority
        let collection_key = self.collection.key();
        let signer_seeds = &[
            UPDATE_AUTHORITY_SEED,
            collection_key.as_ref(),
            &[bumps.update_authority],
        ];

        // A frozen asset rejects burns, even from the BurnDelegate, so we thaw it first
        // The FreezeDelegate authority is the update authority (PDA of the program), see stake
        UpdatePluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.update_authority.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
            .invoke_signed(&[signer_seeds])?;

        // Burn the asset as the BurnDelegate added on stake (its authority is the update authority)
        BurnV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.update_authority.to_account_info()))
            .system_program(Some(&self.system_program.to_account_info()))
            .invoke_signed(&[signer_seeds])?;

        // The burned asset leaves the staking: same decrement as unstake, otherwise the counter drifts
        update_total_staked(
            -1,
            &self.collection.to_account_info(),
            &self.owner.to_account_info(),
            &self.update_authority.to_account_info(),
            &self.system_program.to_account_info(),
            &self.mpl_core_program.to_account_info(),
            signer_seeds,
        )?;

        // Prepare signer seeds for config PDA
        let config_seeds = &[CONFIG_SEED, collection_key.as_ref(), &[self.config.bump]];
        let config_signer_seeds = &[&config_seeds[..]];

        mint_to_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                MintToChecked {
                    mint: self.rewards_mint.to_account_info(),
                    to: self.user_rewards_ata.to_account_info(),
                    authority: self.config.to_account_info(),
                },
                config_signer_seeds,
            ),
            amount,
            self.rewards_mint.decimals,
        )?;

        Ok(())
    }
}
