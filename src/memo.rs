#[cfg(test)]
mod tests;

use crate::address::Address;
use crate::eth_logs::MintEvent;
use crate::eth_rpc::Hash;
use crate::numeric::LogIndex;
use icrc_ledger_types::icrc1::transfer::Memo;
use minicbor::{Decode, Encode, Encoder};

/// Encodes minter memo as a binary blob.
fn encode<T: minicbor::Encode<()>>(t: &T) -> Vec<u8> {
    let mut encoder = Encoder::new(Vec::new());
    encoder.encode(t).expect("minicbor encoding failed");
    encoder.into_writer()
}

#[derive(Decode, Encode, Debug, Eq, PartialEq)]
pub enum MintMemo {
    #[n(0)]
    /// The minter received some ETH.
    Convert {
        #[n(0)]
        /// The sender of the ETH.
        from_address: Address,
        #[n(1)]
        /// Hash of the transaction.
        tx_hash: Hash,
        #[n(2)]
        log_index: LogIndex,
    },
    #[n(1)]
    Reimburse {
        #[n(0)]
        /// The id corresponding to the withdrawal request.
        withdrawal_id: u64,
        #[n(1)]
        /// Hash of the failed transaction.
        tx_hash: Hash,
    },
}

impl From<MintMemo> for Memo {
    fn from(value: MintMemo) -> Self {
        Memo::from(encode(&value))
    }
}

#[derive(Decode, Encode, Debug, Eq, PartialEq)]
pub enum BurnMemo {
    #[n(0)]
    /// The minter processed a withdraw request.
    Convert {
        #[n(0)]
        /// The destination of the withdraw request.
        to_address: Address,
    },
}

impl From<BurnMemo> for Memo {
    fn from(value: BurnMemo) -> Self {
        Memo::from(encode(&value))
    }
}

impl From<MintEvent> for Memo {
    fn from(event: MintEvent) -> Self {
        Memo::from(MintMemo::Convert {
            from_address: event.from_address,
            tx_hash: event.transaction_hash,
            log_index: event.log_index,
        })
    }
}
