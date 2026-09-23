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
        // We start by fetching the existing attributes (if they exist)
        let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseAssetV1, Attributes>(
            &self.asset.to_account_info(),
            PluginType::Attributes,
        )
        .ok()
        .map(|(_, attrs, _)| attrs);

        // If the attributes don't exist, we return an error
        require!(
            attributes_fetched.is_some(),
            CoreStakingError::AssetNotStaked
        );

        // Prepare the Attributes list to update based on the existing attributes
        let attributes = attributes_fetched.unwrap();
        let mut attributes_list: Vec<Attribute> =
            Vec::with_capacity(attributes.attribute_list.len());

        let current_timestamp = Clock::get()?.unix_timestamp;
        let mut staked_timestamp: i64 = 0;
        let mut last_claimed_timestamp: i64 = 0;
        let mut last_claimed_found = false;

        for attribute in &attributes.attribute_list {
            if attribute.key == "staked" {
                require!(attribute.value == "true", CoreStakingError::AssetNotStaked);
            } else if attribute.key == "staked_at" {
                staked_timestamp = attribute
                    .value
                    .parse::<i64>()
                    .map_err(|_| CoreStakingError::InvalidTimestamp)?;
            } else if attribute.key == "last_claimed_at" {
                last_claimed_timestamp = attribute
                    .value
                    .parse::<i64>()
                    .map_err(|_| CoreStakingError::InvalidTimestamp)?;
                last_claimed_found = true;
            } else {
                attributes_list.push(attribute.clone());
            }
        }

        if !last_claimed_found {
            last_claimed_timestamp = staked_timestamp;
        }

        attributes_list.push(Attribute {
            key: "staked".to_string(),
            value: "true".to_string(),
        });
        attributes_list.push(Attribute {
            key: "staked_at".to_string(),
            value: staked_timestamp.to_string(),
        });

        // Calculate elapsed time (in seconds) since the last claim/stake
        let elapsed_time = current_timestamp
            .checked_sub(last_claimed_timestamp)
            .ok_or(CoreStakingError::InvalidTimestamp)?;
        // Elapsed time in days
        let elapsed_days = elapsed_time
            .checked_div(SECONDS_PER_DAY)
            .ok_or(CoreStakingError::InvalidTimestamp)?;

        // We must have at least one day elapsed to claim
        require!(elapsed_days > 0, CoreStakingError::NoRewardsToClaim);

        // last_claimed_at is set to the last claimed timestamp + elapsed days in seconds to preserve fractional seconds
        let new_last_claimed = last_claimed_timestamp
            .checked_add(
                elapsed_days
                    .checked_mul(SECONDS_PER_DAY)
                    .ok_or(CoreStakingError::InvalidTimestamp)?,
            )
            .ok_or(CoreStakingError::InvalidTimestamp)?;

        attributes_list.push(Attribute {
            key: "last_claimed_at".to_string(),
            value: new_last_claimed.to_string(),
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
