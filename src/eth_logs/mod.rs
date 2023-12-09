#[cfg(test)]
mod tests;

use crate::address::Address;
use crate::eth_rpc::{FixedSizeData, Hash, LogEntry};
use crate::eth_rpc_client::{EthRpcClient, MultiCallError};
use crate::logs::{DEBUG, INFO};
use crate::numeric::{BlockNumber, LogIndex, TokenId};
use crate::state::read_state;
use candid::Principal;
use ethnum::u256;
use hex_literal::hex;
use ic_canister_log::log;
use minicbor::{Decode, Encode};
use std::fmt;
use thiserror::Error;

pub(crate) const TRANSFER_EVENT_TOPIC: [u8; 32] =
    hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct TransferEvent {
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
    #[n(5)]
    pub token_id: TokenId,
}

impl fmt::Debug for TransferEvent {
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

impl TransferEvent {
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
) -> Result<(Vec<TransferEvent>, Vec<TransferEventError>), MultiCallError<Vec<LogEntry>>> {
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
            topics: vec![FixedSizeData(TRANSFER_EVENT_TOPIC)],
        })
        .await?;

    let (ok, not_ok): (Vec<_>, Vec<_>) = result
        .into_iter()
        .map(TransferEvent::try_from)
        .partition(Result::is_ok);
    let valid_transactions: Vec<TransferEvent> = ok.into_iter().map(Result::unwrap).collect();
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

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum EventSourceError {
    #[error("failed to decode principal from bytes {invalid_principal}")]
    InvalidPrincipal { invalid_principal: FixedSizeData },
    #[error("invalid ReceivedEthEvent: {0}")]
    InvalidEvent(String),
}

impl TryFrom<LogEntry> for TransferEvent {
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

        if entry.topics.len() != 3 {
            return Err(TransferEventError::InvalidEventSource {
                source: event_source,
                error: EventSourceError::InvalidEvent(format!(
                    "Expected exactly 3 topics, got {}",
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
        // TODO: check that the token id is a valid u256
        let token_id = TokenId::from_be_bytes(entry.topics[3].0);

        Ok(TransferEvent {
            transaction_hash,
            block_number,
            log_index,
            from_address,
            to_address,
            token_id,
        })
    }
}

/// Decode a candid::Principal from a slice of at most 32 bytes
/// encoded as follows
/// - the first byte is the number of bytes in the principal
/// - the next N bytes are the principal
/// - the remaining bytes are zero
///
/// Any other encoding will return an error.
/// Some specific valid [`Principal`]s are also not allowed
/// since the decoded principal will be used to receive ckETH:
/// * the management canister principal
/// * the anonymous principal
///
/// This method MUST never panic (decode bytes from untrusted sources).
fn parse_principal_from_slice(slice: &[u8]) -> Result<Principal, String> {
    const ANONYMOUS_PRINCIPAL_BYTES: [u8; 1] = [4];

    if slice.is_empty() {
        return Err("slice too short".to_string());
    }
    if slice.len() > 32 {
        return Err(format!("Expected at most 32 bytes, got {}", slice.len()));
    }
    let num_bytes = slice[0] as usize;
    if num_bytes == 0 {
        return Err("management canister principal is not allowed".to_string());
    }
    if num_bytes > 29 {
        return Err(format!(
            "invalid number of bytes: expected a number in the range [1,29], got {num_bytes}",
        ));
    }
    if slice.len() < 1 + num_bytes {
        return Err("slice too short".to_string());
    }
    let (principal_bytes, trailing_zeroes) = slice[1..].split_at(num_bytes);
    if !trailing_zeroes
        .iter()
        .all(|trailing_zero| *trailing_zero == 0)
    {
        return Err("trailing non-zero bytes".to_string());
    }
    if principal_bytes == ANONYMOUS_PRINCIPAL_BYTES {
        return Err("anonymous principal is not allowed".to_string());
    }
    Principal::try_from_slice(principal_bytes).map_err(|err| err.to_string())
}
