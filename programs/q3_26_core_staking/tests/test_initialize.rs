mod common;

use anchor_lang::solana_program::program_option::COption;
use common::{
    assert_failed_with, assert_program_error, config_pda, initialize_instruction, succeed, Env,
    Scenario, FREEZE_PERIOD, REWARDS_BPS,
};
use mpl_core::instructions::CreateCollectionV2Builder;
use q3_26_core_staking::error::CoreStakingError;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn stores_the_config_and_creates_the_rewards_mint() {
    let scenario = Scenario::new();

    let config = scenario.env.config(&scenario.collection).unwrap();
    assert_eq!(config.rewards_bps, REWARDS_BPS);
    assert_eq!(config.freeze_period, FREEZE_PERIOD);

    let mint = scenario.env.rewards_mint(&scenario.collection).unwrap();
    assert_eq!(mint.decimals, 6);
    assert_eq!(mint.supply, 0);

    assert_eq!(
        mint.mint_authority,
        COption::Some(config_pda(&scenario.collection))
    );
}

#[test]
fn cannot_be_initialized_twice() {
    let mut scenario = Scenario::new();
    let admin = scenario.env.payer.pubkey();

    let failed = scenario
        .env
        .send(
            &[initialize_instruction(&admin, &scenario.collection, 1, 1)],
            &[],
        )
        .unwrap_err();
    assert_failed_with(&failed, "Custom(0)");
}

#[test]
fn rejects_a_collection_the_program_does_not_control() {
    let mut env = Env::new();
    let admin = env.payer.pubkey();
    let collection = Keypair::new();

    let create = CreateCollectionV2Builder::new()
        .collection(collection.pubkey())
        .payer(admin)
        .name("Foreign Collection".to_string())
        .uri("https://example.com/foreign.json".to_string())
        .instruction();
    succeed(env.send(&[create], &[&collection]));

    let failed = env
        .send(
            &[initialize_instruction(
                &admin,
                &collection.pubkey(),
                REWARDS_BPS,
                FREEZE_PERIOD,
            )],
            &[],
        )
        .unwrap_err();
    assert_program_error(&failed, CoreStakingError::InvalidUpdateAuthority);
}
