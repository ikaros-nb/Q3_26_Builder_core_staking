mod common;

use common::{assert_program_error, days, Scenario, DAY, FREEZE_PERIOD, TOKEN};
use q3_26_core_staking::error::CoreStakingError;

#[test]
fn rejects_before_the_freeze_period() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD) - 1);

    let failed = scenario.try_unstake().unwrap_err();
    assert_program_error(&failed, CoreStakingError::FreezePeriodNotElapsed);
}

#[test]
fn pays_rewards_and_releases_the_asset() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD));

    scenario.unstake();

    assert_eq!(scenario.rewards(), FREEZE_PERIOD as u64 * TOKEN);
    assert_eq!(scenario.attribute("staked").as_deref(), Some("false"));
    assert_eq!(scenario.attribute("staked_at").as_deref(), Some("0"));
    // Both delegates are gone: the owner has full control again
    let plugins = scenario.env.asset(&scenario.asset).unwrap().plugin_list;
    assert!(plugins.freeze_delegate.is_none());
    assert!(plugins.burn_delegate.is_none());
}

#[test]
fn decrements_the_collection_total_staked() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD));

    scenario.unstake();

    assert_eq!(scenario.total_staked(), Some(0));
}

#[test]
fn the_asset_can_be_staked_again() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD));
    scenario.unstake();

    scenario.stake();

    assert_eq!(scenario.attribute("staked").as_deref(), Some("true"));
    assert!(scenario.is_frozen());
    assert_eq!(scenario.total_staked(), Some(1));
}

#[test]
fn a_claim_restarts_the_freeze_period() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD - 1));
    scenario.claim();
    // FREEZE_PERIOD days since stake, but only one since the claim
    scenario.env.warp(DAY);

    let failed = scenario.try_unstake().unwrap_err();
    assert_program_error(&failed, CoreStakingError::FreezePeriodNotElapsed);
}

#[test]
fn cannot_unstake_twice() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(days(FREEZE_PERIOD));
    scenario.unstake();

    let failed = scenario.try_unstake().unwrap_err();
    assert_program_error(&failed, CoreStakingError::AssetNotStaked);
}
