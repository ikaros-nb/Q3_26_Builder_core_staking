mod common;

use common::{assert_program_error, days, Scenario, DAY, TOKEN};
use q3_26_core_staking::error::CoreStakingError;

#[test]
fn rejects_before_a_full_day() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(DAY - 1);

    let failed = scenario.try_claim().unwrap_err();
    assert_program_error(&failed, CoreStakingError::NoRewardsToClaim);
}

#[test]
fn pays_whole_days_and_keeps_the_asset_staked() {
    let mut scenario = Scenario::staked();
    let staked_at = scenario.env.unix_timestamp();
    scenario.env.warp(days(3) + DAY / 2);

    scenario.claim();

    assert_eq!(scenario.rewards(), 3 * TOKEN);

    assert_eq!(
        scenario.attribute("staked_at"),
        Some((staked_at + days(3)).to_string())
    );
    assert_eq!(scenario.attribute("staked").as_deref(), Some("true"));
    assert!(scenario.is_frozen());
}

#[test]
fn the_kept_remainder_counts_toward_the_next_claim() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(DAY + DAY / 2);
    scenario.claim();
    scenario.env.warp(DAY / 2);

    scenario.claim();

    assert_eq!(scenario.rewards(), 2 * TOKEN);
}

#[test]
fn cannot_claim_the_same_days_twice() {
    let mut scenario = Scenario::staked();
    scenario.env.warp(DAY);
    scenario.claim();

    let failed = scenario.try_claim().unwrap_err();
    assert_program_error(&failed, CoreStakingError::NoRewardsToClaim);
    assert_eq!(scenario.rewards(), TOKEN);
}

#[test]
fn rejects_an_asset_that_is_not_staked() {
    let mut scenario = Scenario::new();
    scenario.env.warp(DAY);

    let failed = scenario.try_claim().unwrap_err();
    assert_program_error(&failed, CoreStakingError::AssetNotStaked);
}
