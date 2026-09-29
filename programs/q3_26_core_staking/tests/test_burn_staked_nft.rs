mod common;

use common::{
    assert_program_error, burn_staked_nft_instruction, days, Scenario, FREEZE_PERIOD, TOKEN,
};
use q3_26_core_staking::{error::CoreStakingError, BURN_BONUS};

#[test]
fn rejects_before_the_freeze_period() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD) - 1);

    let failed = scenario.try_burn().unwrap_err();
    assert_program_error(&failed, CoreStakingError::FreezePeriodNotElapsed);
}

#[test]
fn burns_the_asset_and_pays_rewards_plus_the_bonus() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD));

    scenario.burn();

    assert_eq!(
        scenario.rewards(),
        (FREEZE_PERIOD as u64 + BURN_BONUS) * TOKEN
    );
    assert!(scenario.env.asset(&scenario.asset).is_none());
    let collection = scenario.env.collection(&scenario.collection).unwrap();
    assert_eq!(collection.current_size, 0);
}

#[test]
fn decrements_the_collection_total_staked() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD));

    scenario.burn();

    assert_eq!(scenario.total_staked(), Some(0));
}

#[test]
fn rejects_an_asset_that_is_not_staked() {
    let mut scenario = Scenario::new();
    scenario.env.warp(days(FREEZE_PERIOD));

    let failed = scenario.try_burn().unwrap_err();
    assert_program_error(&failed, CoreStakingError::AssetNotStaked);
}

#[test]
fn rejects_a_signer_who_does_not_own_the_asset() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD));

    let failed = scenario
        .try_as_stranger(burn_staked_nft_instruction)
        .unwrap_err();
    assert_program_error(&failed, CoreStakingError::InvalidOwner);
    assert!(scenario.env.asset(&scenario.asset).is_some());
}
