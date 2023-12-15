#[cfg(test)]
mod tests;

use askama::Template;
use ic_cketh_minter::eth_logs::MintEvent;
use ic_cketh_minter::lifecycle::EthereumNetwork;
use ic_cketh_minter::numeric::BlockNumber;
use ic_cketh_minter::state::{MintedEvent, State};
use std::cmp::Reverse;
use std::collections::BTreeSet;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub ethereum_network: EthereumNetwork,
    pub contract_address: String,
    pub minter_address: String,
    pub first_synced_block: BlockNumber,
    pub last_synced_block: BlockNumber,
    pub last_observed_block: Option<BlockNumber>,
    pub minted_events: Vec<MintedEvent>,
    pub events_to_mint: Vec<MintEvent>,
    pub skipped_blocks: BTreeSet<BlockNumber>,
}

impl DashboardTemplate {
    pub fn from_state(state: &State) -> Self {
        let mut minted_events: Vec<_> = state.minted_events.values().cloned().collect();
        minted_events.sort_unstable_by_key(|event| Reverse(event.mint_event.token_id));
        let mut events_to_mint: Vec<_> = state.events_to_mint.values().cloned().collect();
        events_to_mint.sort_unstable_by_key(|event| Reverse(event.block_number));

        DashboardTemplate {
            ethereum_network: state.ethereum_network,
            contract_address: state.ethereum_contract_address.to_string(),
            minter_address: state.minter_address.to_string(),
            first_synced_block: state.first_scraped_block_number,
            last_synced_block: state.last_scraped_block_number,
            last_observed_block: state.last_observed_block_number,
            minted_events,
            events_to_mint,
            skipped_blocks: state.skipped_blocks.clone(),
        }
    }
}
