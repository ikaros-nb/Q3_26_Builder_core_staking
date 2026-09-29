use crate::error::CoreStakingError;
use anchor_lang::prelude::*;
use mpl_core::{
    accounts::BaseCollectionV1,
    fetch_plugin,
    instructions::{AddCollectionPluginV1CpiBuilder, UpdateCollectionPluginV1CpiBuilder},
    types::{Attribute, Attributes, Plugin, PluginAuthority, PluginType},
};

pub const TOTAL_STAKED_KEY: &str = "total_staked";

/// Adds `delta` to the "total_staked" Attribute of the collection
/// The Attributes Plugin is created on the first stake, starting from 0
pub fn update_total_staked<'info>(
    delta: i64,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    update_authority: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    let attributes_fetched: Option<Attributes> =
        fetch_plugin::<BaseCollectionV1, Attributes>(collection, PluginType::Attributes)
            .ok()
            .map(|(_, attrs, _)| attrs);

    let mut attributes_list: Vec<Attribute> = Vec::new();
    let mut total_staked: u64 = 0;
    if let Some(attributes) = &attributes_fetched {
        for attribute in &attributes.attribute_list {
            if attribute.key == TOTAL_STAKED_KEY {
                total_staked = attribute
                    .value
                    .parse::<u64>()
                    .map_err(|_| CoreStakingError::InvalidTotalStaked)?;
            } else {
                attributes_list.push(attribute.clone());
            }
        }
    }

    let total_staked = total_staked
        .checked_add_signed(delta)
        .ok_or(CoreStakingError::InvalidTotalStaked)?;
    attributes_list.push(Attribute {
        key: TOTAL_STAKED_KEY.to_string(),
        value: total_staked.to_string(),
    });
    let plugin = Plugin::Attributes(Attributes {
        attribute_list: attributes_list,
    });

    if attributes_fetched.is_none() {
        AddCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .init_authority(PluginAuthority::UpdateAuthority)
            .invoke_signed(&[signer_seeds])?;
    } else {
        UpdateCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .invoke_signed(&[signer_seeds])?;
    }

    Ok(())
}
