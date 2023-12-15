#[cfg(test)]
mod tests;

use crate::address::Address;
use crate::eth_rpc::{FixedSizeData, Hash, LogEntry};
use crate::eth_rpc_client::{EthRpcClient, MultiCallError};
use crate::logs::{DEBUG, INFO};
use crate::numeric::{BlockNumber, LogIndex};
use crate::state::read_state;

use ethnum::u256;
use hex_literal::hex;
use ic_canister_log::log;
use minicbor::{Decode, Encode};
use std::fmt;
use thiserror::Error;

pub(crate) const TRANSFER_EVENT_TOPIC: [u8; 32] =
    hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct MintEvent {
    #[n(0)]
    pub transaction_hash: Hash,
    #[n(1)]
    pub block_number: BlockNumber,
    #[cbor(n(2))]
    pub log_index: LogIndex,
    #[n(3)]
    pub from_address: Address,
    #[n(4)]
    pub to_address: Address,
    #[cbor(n(5), with = "crate::cbor::u256")]
    pub token_id: u256,
}

impl fmt::Debug for MintEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReceivedEthEvent")
            .field("transaction_hash", &self.transaction_hash)
            .field("block_number", &self.block_number)
            .field("log_index", &self.log_index)
            .field("from_address", &self.from_address)
            .field("to_address", &self.from_address)
            .field("token_id", &self.token_id)
            .finish()
    }
}

/// A unique identifier of the event source: the source transaction hash and the log
/// entry index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct EventSource {
    #[n(0)]
    pub transaction_hash: Hash,
    #[n(1)]
    pub log_index: LogIndex,
}

impl fmt::Display for EventSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}:{}", self.transaction_hash, self.log_index)
    }
}

impl MintEvent {
    pub fn source(&self) -> EventSource {
        EventSource {
            transaction_hash: self.transaction_hash,
            log_index: self.log_index,
        }
    }
}

pub async fn last_received_eth_events(
    contract_address: Address,
    from: BlockNumber,
    to: BlockNumber,
) -> Result<(Vec<MintEvent>, Vec<TransferEventError>), MultiCallError<Vec<LogEntry>>> {
    use crate::eth_rpc::GetLogsParam;

    if from > to {
        ic_cdk::trap(&format!(
            "BUG: invalid block range. {:?} should not be greater than {:?}",
            from, to
        ));
    }

    let result = read_state(EthRpcClient::from_state)
        .eth_get_logs(GetLogsParam {
            from_block: from.into(),
            to_block: to.into(),
            address: vec![contract_address],
            topics: vec![
                FixedSizeData(TRANSFER_EVENT_TOPIC),
                Address::ZERO.to_fixed_size_data(), // this ensures we only receive mint events
            ],
        })
        .await?;

    let (ok, not_ok): (Vec<_>, Vec<_>) = result
        .into_iter()
        .map(MintEvent::try_from)
        .partition(Result::is_ok);
    let valid_transactions: Vec<MintEvent> = ok.into_iter().map(Result::unwrap).collect();
    let errors: Vec<TransferEventError> = not_ok.into_iter().map(Result::unwrap_err).collect();
    Ok((valid_transactions, errors))
}

pub fn report_transaction_error(error: TransferEventError) {
    match error {
        TransferEventError::PendingLogEntry => {
            log!(
                DEBUG,
                "[report_transaction_error]: ignoring pending log entry",
            );
        }
        TransferEventError::InvalidEventSource { source, error } => {
            log!(
                INFO,
                "[report_transaction_error]: cannot process {source} due to {error}",
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferEventError {
    PendingLogEntry,
    InvalidEventSource {
        source: EventSource,
        error: EventSourceError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintEventError {
    NoMintEvent,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum EventSourceError {
    #[error("failed to decode principal from bytes {invalid_principal}")]
    InvalidPrincipal { invalid_principal: FixedSizeData },
    #[error("invalid ReceivedEthEvent: {0}")]
    InvalidEvent(String),
}

impl TryFrom<LogEntry> for MintEvent {
    type Error = TransferEventError;

    fn try_from(entry: LogEntry) -> Result<Self, Self::Error> {
        let _block_hash = entry
            .block_hash
            .ok_or(TransferEventError::PendingLogEntry)?;
        let block_number = entry
            .block_number
            .ok_or(TransferEventError::PendingLogEntry)?;
        let transaction_hash = entry
            .transaction_hash
            .ok_or(TransferEventError::PendingLogEntry)?;
        let _transaction_index = entry
            .transaction_index
            .ok_or(TransferEventError::PendingLogEntry)?;
        let log_index = entry.log_index.ok_or(TransferEventError::PendingLogEntry)?;
        let event_source = EventSource {
            transaction_hash,
            log_index,
        };

        if entry.removed {
            return Err(TransferEventError::InvalidEventSource {
                source: event_source,
                error: EventSourceError::InvalidEvent(
                    "this event has been removed from the chain".to_string(),
                ),
            });
        }

        if entry.topics.len() != 4 {
            return Err(TransferEventError::InvalidEventSource {
                source: event_source,
                error: EventSourceError::InvalidEvent(format!(
                    "Expected exactly 4 topics, got {}",
                    entry.topics.len()
                )),
            });
        }
        let from_address = Address::try_from(&entry.topics[1].0).map_err(|err| {
            TransferEventError::InvalidEventSource {
                source: event_source,
                error: EventSourceError::InvalidEvent(format!(
                    "Invalid address in log entry: {}",
                    err
                )),
            }
        })?;
        let to_address = Address::try_from(&entry.topics[2].0).map_err(|err| {
            TransferEventError::InvalidEventSource {
                source: event_source,
                error: EventSourceError::InvalidEvent(format!(
                    "Invalid address in log entry: {}",
                    err
                )),
            }
        })?;
        let token_id = u256::from_be_bytes(entry.topics[3].0);

        Ok(MintEvent {
            transaction_hash,
            block_number,
            log_index,
            from_address,
            to_address,
            token_id,
        })
    }
}

// impl TryFrom<TransferEvent> for MintEvent {
//     type Error = MintEventError;

//     fn try_from(transfer_event: TransferEvent) -> Result<Self, Self::Error> {
//         // check if from_address is the zero address
//         if transfer_event.from_address != Address::ZERO {
//             return Err(MintEventError::NoMintEvent);
//         }

//         Ok(MintEvent(transfer_event))
//     }
// }

// impl Deref for MintEvent {
//     type Target = TransferEvent;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }
