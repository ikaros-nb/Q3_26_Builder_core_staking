mod common;

use common::{update_authority_pda, Scenario};

#[test]
fn the_program_pda_is_the_update_authority() {
    let scenario = Scenario::new();

    let collection = scenario.env.collection(&scenario.collection).unwrap();
    assert_eq!(
        collection.update_authority,
        update_authority_pda(&scenario.collection)
    );
    assert_eq!(collection.name, "Staking Collection");
}
