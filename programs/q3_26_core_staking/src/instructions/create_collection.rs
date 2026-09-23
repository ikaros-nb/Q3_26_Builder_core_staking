use anchor_lang::prelude::*;
use mpl_core::{
    ID as MPL_CORE_ID,
    instructions::CreateCollectionV2CpiBuilder,
};

use crate::constants::UPDATE_AUTHORITY_SEED;

#[derive(Accounts)]
pub struct CreateCollection<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub collection: Signer<'info>,
    /// CHECK: This account is not initialized and is being used for signing purposes only, we verify that derives from the correct seeds
    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: This is the ID of the MPL Core Program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
}

impl<'info> CreateCollection<'info> {
    pub fn create_collection(
        &self,
        name: String,
        uri: String,
        bumps: &CreateCollectionBumps,
    ) -> Result<()> {
        // Signer seeds for the update authority
        let collection_key = self.collection.key();
        let signer_seeds = &[
            UPDATE_AUTHORITY_SEED,
            collection_key.as_ref(),
            &[bumps.update_authority],
        ];

        CreateCollectionV2CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .collection(&self.collection.to_account_info())
            .payer(&self.payer.to_account_info())
            .update_authority(Some(&self.update_authority.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .name(name)
            .uri(uri)
            .invoke_signed(&[signer_seeds])?;

        Ok(())
    }
}
