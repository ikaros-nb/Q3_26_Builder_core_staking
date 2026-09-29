mod common;

use common::Scenario;
use mpl_core::types::UpdateAuthority;
use solana_signer::Signer;

#[test]
fn mints_an_unstaked_asset_into_the_collection() {
    let scenario = Scenario::new();

    let asset = scenario.env.asset(&scenario.asset).unwrap();
    assert_eq!(asset.base.owner, scenario.user.pubkey());
    assert_eq!(
        asset.base.update_authority,
        UpdateAuthority::Collection(scenario.collection)
    );

    assert!(asset.plugin_list.attributes.is_none());
    assert!(asset.plugin_list.freeze_delegate.is_none());

    let collection = scenario.env.collection(&scenario.collection).unwrap();
    assert_eq!(collection.num_minted, 1);
    assert_eq!(collection.current_size, 1);
}
