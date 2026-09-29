#![allow(dead_code)]

mod requests;
mod scenario;
pub use requests::*;
pub use scenario::*;

use anchor_lang::prelude::{Clock, Pubkey};
use anchor_lang::solana_program::instruction::{error::InstructionError, Instruction};
use anchor_lang::AccountDeserialize;
use anchor_spl::token::{Mint, TokenAccount};
use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    LiteSVM,
};
use mpl_core::{accounts::BaseCollectionV1, errors::MplCoreError, Asset};
use q3_26_core_staking::{error::CoreStakingError, state::Config};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction_error::TransactionError;

/// 2026-01-01T00:00:00Z
pub const START_TIMESTAMP: i64 = 1_767_225_600;

pub struct Env {
    pub svm: LiteSVM,
    pub payer: Keypair,
}

impl Env {
    pub fn new() -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program(
            q3_26_core_staking::id(),
            include_bytes!(concat!(
                env!("CARGO_TARGET_TMPDIR"),
                "/../deploy/q3_26_core_staking.so"
            )),
        )
        .unwrap();
        // Dumped from mainnet: solana program dump -u m CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d
        svm.add_program(mpl_core::ID, include_bytes!("../fixtures/mpl_core.so"))
            .unwrap();

        // Start from a real date: at 0, `staked_at = 0` would look like the reset value of unstake.
        let mut clock = svm.get_sysvar::<Clock>();
        clock.unix_timestamp = START_TIMESTAMP;
        svm.set_sysvar(&clock);

        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

        Self { svm, payer }
    }

    /// The payer pays the fees and signs first; `signers` add the other signatures.
    pub fn send(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) -> TransactionResult {
        self.svm.expire_blockhash();
        let blockhash = self.svm.latest_blockhash();
        let message =
            Message::new_with_blockhash(instructions, Some(&self.payer.pubkey()), &blockhash);
        let mut keypairs = vec![&self.payer];
        keypairs.extend_from_slice(signers);
        let tx =
            VersionedTransaction::try_new(VersionedMessage::Legacy(message), &keypairs).unwrap();
        self.svm.send_transaction(tx)
    }

    /// A new keypair with lamports for rent: the owner pays for the plugins it adds.
    pub fn funded_keypair(&mut self) -> Keypair {
        let keypair = Keypair::new();
        self.svm.airdrop(&keypair.pubkey(), 1_000_000_000).unwrap();
        keypair
    }

    /// Moves the Clock sysvar forward: the program reads its time from it.
    pub fn warp(&mut self, seconds: i64) {
        let mut clock = self.svm.get_sysvar::<Clock>();
        clock.unix_timestamp += seconds;
        self.svm.set_sysvar(&clock);
    }

    pub fn unix_timestamp(&self) -> i64 {
        self.svm.get_sysvar::<Clock>().unix_timestamp
    }

    fn deserialize<T: AccountDeserialize>(&self, address: &Pubkey) -> Option<T> {
        let account = self.svm.get_account(address)?;
        T::try_deserialize(&mut account.data.as_slice()).ok()
    }

    pub fn config(&self, collection: &Pubkey) -> Option<Config> {
        self.deserialize(&config_pda(collection))
    }

    pub fn rewards_mint(&self, collection: &Pubkey) -> Option<Mint> {
        self.deserialize(&rewards_mint_pda(collection))
    }

    /// Rewards held by `owner`, 0 if the ATA does not exist.
    pub fn rewards(&self, owner: &Pubkey, collection: &Pubkey) -> u64 {
        self.deserialize::<TokenAccount>(&rewards_ata(owner, collection))
            .map_or(0, |ata| ata.amount)
    }

    /// The asset with its plugins, `None` once burned.
    pub fn asset(&self, asset: &Pubkey) -> Option<Box<Asset>> {
        let account = self.svm.get_account(asset)?;
        Asset::deserialize(&account.data).ok()
    }

    pub fn collection(&self, collection: &Pubkey) -> Option<BaseCollectionV1> {
        let account = self.svm.get_account(collection)?;
        BaseCollectionV1::from_bytes(&account.data).ok()
    }

    /// Value of one key of the Attributes plugin.
    pub fn attribute(&self, asset: &Pubkey, key: &str) -> Option<String> {
        self.asset(asset)?
            .plugin_list
            .attributes?
            .attributes
            .attribute_list
            .into_iter()
            .find(|attribute| attribute.key == key)
            .map(|attribute| attribute.value)
    }
}

/// Happy path: panics with the logs if the transaction fails.
pub fn succeed(result: TransactionResult) -> TransactionMetadata {
    result.unwrap_or_else(|failed| panic!("{:?}\n{:#?}", failed.err, failed.meta.logs))
}

/// Exact check: the program refused with this error.
pub fn assert_program_error(failed: &FailedTransactionMetadata, expected: CoreStakingError) {
    assert_custom_error(failed, expected.into(), &format!("{expected:?}"));
}

/// Exact check: MPL Core, called directly, refused with this error.
pub fn assert_mpl_core_error(failed: &FailedTransactionMetadata, expected: MplCoreError) {
    assert_custom_error(failed, expected.clone() as u32, &format!("{expected:?}"));
}

fn assert_custom_error(failed: &FailedTransactionMetadata, code: u32, name: &str) {
    let expected_err =
        TransactionError::InstructionError(0, InstructionError::Custom(code));
    assert_eq!(
        failed.err, expected_err,
        "expected {name}\n{:#?}",
        failed.meta.logs
    );
}

/// Loose check on the Debug output, for errors raised by the runtime or by
/// another program (`Custom(0)` from System, an MPL Core error, ...).
pub fn assert_failed_with(failed: &FailedTransactionMetadata, expected: &str) {
    let actual = format!("{:?}", failed.err);
    assert!(
        actual.contains(expected),
        "expected {expected}, got {actual}\n{:#?}",
        failed.meta.logs
    );
}
