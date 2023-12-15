use crate::address::Address;
use crate::endpoints::CandidBlockTag;
use crate::eth_rpc::BlockTag;
use crate::lifecycle::EthereumNetwork;
use crate::numeric::{BlockNumber, TransactionNonce, Wei};
use crate::state::{InvalidStateError, State};
use candid::types::number::Nat;
use candid::types::principal::Principal;
use candid::{CandidType, Deserialize};
use minicbor::{Decode, Encode};

#[derive(CandidType, Deserialize, Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct InitArg {
    #[n(0)]
    pub ethereum_network: EthereumNetwork,
    #[n(1)]
    pub minter_address: Option<String>,
    #[n(2)]
    pub ethereum_contract_address: String,
    #[n(3)]
    pub ethereum_block_height: CandidBlockTag,
    #[cbor(n(4), with = "crate::cbor::nat")]
    pub last_scraped_block_number: Nat,
}

impl TryFrom<InitArg> for State {
    type Error = InvalidStateError;

    fn try_from(
        InitArg {
            ethereum_network,
            minter_address,
            ethereum_contract_address,
            ethereum_block_height,
            last_scraped_block_number,
        }: InitArg,
    ) -> Result<Self, Self::Error> {
        use std::str::FromStr;

        let ethereum_contract_address =
            Address::from_str(&ethereum_contract_address).map_err(|e| {
                InvalidStateError::InvalidEthereumContractAddress(format!("ERROR: {}", e))
            })?;
        let minter_address = minter_address.map_or_else(
            || Ok(Address::ZERO), // Provides a default value when minter_address is None
            |a| {
                Address::from_str(&a) // Tries to parse the address from the string when it's Some
                    .map_err(|e| {
                        InvalidStateError::InvalidMinterAddress(format!("ERROR: {}", e))
                    })
            },
        )?;

        let last_scraped_block_number =
            BlockNumber::try_from(last_scraped_block_number).map_err(|e| {
                InvalidStateError::InvalidLastScrapedBlockNumber(format!("ERROR: {}", e))
            })?;
        let first_scraped_block_number =
            last_scraped_block_number
                .checked_increment()
                .ok_or_else(|| {
                    InvalidStateError::InvalidLastScrapedBlockNumber(
                        "ERROR: last_scraped_block_number is at maximum value".to_string(),
                    )
                })?;
        let state = Self {
            ethereum_network,
            minter_address,
            ethereum_contract_address,
            ethereum_block_height: BlockTag::from(ethereum_block_height),
            first_scraped_block_number,
            last_scraped_block_number,
            last_observed_block_number: None,
            events_to_mint: Default::default(),
            minted_events: Default::default(),
            invalid_events: Default::default(),
            skipped_blocks: Default::default(),
            active_tasks: Default::default(),
            http_request_counter: 0,
        };
        state.validate_config()?;
        Ok(state)
    }
}
