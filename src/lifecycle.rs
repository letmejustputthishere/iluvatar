//! Module dealing with the lifecycle methods of the ckETH Minter.
use crate::lifecycle::init::InitArg;
use crate::lifecycle::upgrade::UpgradeArg;
use candid::{CandidType, Deserialize};
use minicbor::{Decode, Encode};
use std::fmt::{Display, Formatter};

#[cfg(test)]
mod tests;

pub mod init;
pub mod upgrade;
pub use upgrade::post_upgrade;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum MinterArg {
    InitArg(InitArg),
    UpgradeArg(UpgradeArg),
}

#[derive(
    CandidType, Clone, Copy, Default, Deserialize, Debug, Eq, PartialEq, Hash, Encode, Decode,
)]
#[cbor(index_only)]
pub enum Network {
    #[n(1)]
    EthereumMainnet,
    #[n(11155111)]
    #[default]
    EthereumSepolia,
    #[n(43114)]
    AvalancheMainnet,
    #[n(43113)]
    AvalancheFuji,
}

impl Network {
    pub fn chain_id(&self) -> u64 {
        match self {
            Network::EthereumMainnet => 1,
            Network::EthereumSepolia => 11155111,
            Network::AvalancheMainnet => 43114,
            Network::AvalancheFuji => 43113,
        }
    }
}

impl Display for Network {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::EthereumMainnet => write!(f, "Ethereum Mainnet"),
            Network::EthereumSepolia => write!(f, "Ethereum Testnet Sepolia"),
            Network::AvalancheMainnet => write!(f, "Avalanche Mainnet"),
            Network::AvalancheFuji => write!(f, "Avalanche Testnet Fuji"),
        }
    }
}
