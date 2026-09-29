mod common;

use anchor_lang::prelude::Pubkey;
use common::{assert_mpl_core_error, assert_program_error, stake_instruction, Scenario};
use mpl_core::{errors::MplCoreError, instructions::TransferV1Builder, types::PluginAuthority};
use q3_26_core_staking::error::CoreStakingError;
use solana_signer::Signer;

#[test]
fn marks_the_asset_staked_and_freezes_it() {
    let mut scenario = Scenario::new();
    let now = scenario.env.unix_timestamp();

    scenario.stake();

    assert_eq!(scenario.attribute("staked").as_deref(), Some("true"));
    assert_eq!(scenario.attribute("staked_at"), Some(now.to_string()));
    assert!(scenario.is_frozen());

    let plugins = scenario.env.asset(&scenario.asset).unwrap().plugin_list;
    assert_eq!(
        plugins.freeze_delegate.unwrap().base.authority,
        PluginAuthority::UpdateAuthority.into()
    );
    assert_eq!(
        plugins.burn_delegate.unwrap().base.authority,
        PluginAuthority::UpdateAuthority.into()
    );
}

#[test]
fn a_staked_asset_cannot_be_transferred() {
    let mut scenario = Scenario::staked();
    let transfer = TransferV1Builder::new()
        .asset(scenario.asset)
        .collection(Some(scenario.collection))
        .payer(scenario.env.payer.pubkey())
        .authority(Some(scenario.user.pubkey()))
        .new_owner(Pubkey::new_unique())
        .instruction();

    let failed = scenario
        .env
        .send(&[transfer], &[&scenario.user])
        .unwrap_err();

    assert_mpl_core_error(&failed, MplCoreError::InvalidAuthority);
    let asset = scenario.env.asset(&scenario.asset).unwrap();
    assert_eq!(asset.base.owner, scenario.user.pubkey());
}

#[test]
fn rejects_an_asset_already_staked() {
    let mut scenario = Scenario::staked();

    let failed = scenario.try_stake().unwrap_err();
    assert_program_error(&failed, CoreStakingError::AlreadyStaked);
}

#[test]
fn rejects_a_signer_who_does_not_own_the_asset() {
    let mut scenario = Scenario::new();

    let failed = scenario.try_as_stranger(stake_instruction).unwrap_err();
    assert_program_error(&failed, CoreStakingError::InvalidOwner);
}
