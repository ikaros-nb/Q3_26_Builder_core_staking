use crate::{constants::*, error::CoreStakingError, state::Config};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{mint_to_checked, Mint, MintToChecked, TokenAccount, TokenInterface},
};
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::UpdatePluginV1CpiBuilder,
    types::{Attribute, Attributes, Plugin, PluginType, UpdateAuthority},
    ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
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

impl<'info> ClaimRewards<'info> {
    pub fn claim_rewards(&self, bumps: &ClaimRewardsBumps) -> Result<()> {
        // We start by fetching the existing attributes; if the plugin is missing the asset is not staked
        let attributes: Attributes = fetch_plugin::<BaseAssetV1, Attributes>(
            &self.asset.to_account_info(),
            PluginType::Attributes,
        )
        .map(|(_, attrs, _)| attrs)
        .map_err(|_| CoreStakingError::AssetNotStaked)?;

        // Prepare the Attributes list to update based on the existing attributes
        let mut attributes_list: Vec<Attribute> =
            Vec::with_capacity(attributes.attribute_list.len());

        let current_timestamp = Clock::get()?.unix_timestamp;
        let mut staked_found = false;
        let mut staked_timestamp: Option<i64> = None;

        // "staked_at" is the single source of truth for unpaid rewards: stake sets it,
        // claim_rewards advances it by the days it pays, unstake pays from it and resets it.
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
            } else {
                attributes_list.push(attribute.clone());
            }
        }

        // Both staking attributes must be present, otherwise the asset is not staked by this program
        require!(staked_found, CoreStakingError::AssetNotStaked);
        let staked_timestamp = staked_timestamp.ok_or(CoreStakingError::AssetNotStaked)?;

        // Calculate elapsed time (in seconds) since the last checkpoint (stake or previous claim)
        let elapsed_time = current_timestamp
            .checked_sub(staked_timestamp)
            .ok_or(CoreStakingError::InvalidTimestamp)?;
        // Elapsed time in whole days
        let elapsed_days = elapsed_time
            .checked_div(SECONDS_PER_DAY)
            .ok_or(CoreStakingError::InvalidTimestamp)?;

        // We must have at least one full day elapsed to claim
        require!(elapsed_days > 0, CoreStakingError::NoRewardsToClaim);

        // Advance the checkpoint by the exact number of days paid, so the fractional day
        // is carried over to the next claim/unstake instead of being lost.
        // Note: unstake measures the freeze period from this same attribute, so a claim
        // restarts the freeze period.
        let new_staked_timestamp = staked_timestamp
            .checked_add(
                elapsed_days
                    .checked_mul(SECONDS_PER_DAY)
                    .ok_or(CoreStakingError::InvalidTimestamp)?,
            )
            .ok_or(CoreStakingError::InvalidTimestamp)?;

        attributes_list.push(Attribute {
            key: "staked".to_string(),
            value: "true".to_string(),
        });
        attributes_list.push(Attribute {
            key: "staked_at".to_string(),
            value: new_staked_timestamp.to_string(),
        });

        // Prepare signing seeds for the update authority
        let collection_key = self.collection.key();
        let signer_seeds = &[
            UPDATE_AUTHORITY_SEED,
            collection_key.as_ref(),
            &[bumps.update_authority],
        ];

        UpdatePluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.update_authority.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::Attributes(Attributes {
                attribute_list: attributes_list,
            }))
            .invoke_signed(&[signer_seeds])?;

        // Prepare signer seeds for config PDA
        let config_seeds = &[CONFIG_SEED, collection_key.as_ref(), &[self.config.bump]];
        let config_signer_seeds = &[&config_seeds[..]];

        // Calculate the reward amount
        let amount = (elapsed_days as u64)
            .checked_mul(self.config.rewards_bps as u64)
            .ok_or(CoreStakingError::InvalidRewardsBps)?
            .checked_mul(10u64.pow(self.rewards_mint.decimals as u32))
            .ok_or(CoreStakingError::InvalidRewardsBps)?
            .checked_div(10000u64)
            .ok_or(CoreStakingError::InvalidRewardsBps)?;

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
