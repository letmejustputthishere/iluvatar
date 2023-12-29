pub(crate) const ETHEREUM_MAINNET_PROVIDERS: [RpcNodeProvider; 3] = [
    RpcNodeProvider::Ethereum(EthereumProvider::Ankr),
    RpcNodeProvider::Ethereum(EthereumProvider::PublicNode),
    RpcNodeProvider::Ethereum(EthereumProvider::Cloudflare),
];

pub(crate) const ETHEREUM_SEPOLIA_PROVIDERS: [RpcNodeProvider; 2] = [
    RpcNodeProvider::Sepolia(SepoliaProvider::Ankr),
    RpcNodeProvider::Sepolia(SepoliaProvider::PublicNode),
];

pub(crate) const AVALANCHE_MAINNET_PROVIDERS: [RpcNodeProvider; 2] = [
    RpcNodeProvider::Avalanche(AvalancheProvider::Ankr),
    RpcNodeProvider::Avalanche(AvalancheProvider::PublicNode),
];

pub(crate) const AVALANCHE_FUJI_PROVIDERS: [RpcNodeProvider; 2] = [
    RpcNodeProvider::Fuji(FujiProvider::Ankr),
    RpcNodeProvider::Fuji(FujiProvider::PublicNode),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum RpcNodeProvider {
    Ethereum(EthereumProvider),
    Sepolia(SepoliaProvider),
    Avalanche(AvalancheProvider),
    Fuji(FujiProvider),
}

impl RpcNodeProvider {
    pub(crate) fn url(&self) -> &str {
        match self {
            Self::Ethereum(provider) => provider.ethereum_mainnet_endpoint_url(),
            Self::Sepolia(provider) => provider.ethereum_sepolia_endpoint_url(),
            Self::Avalanche(provider) => provider.avalanche_endpoint_url(),
            Self::Fuji(provider) => provider.avalanche_fuji_endpoint_url(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum EthereumProvider {
    // https://www.ankr.com/rpc/
    Ankr,
    // https://publicnode.com/
    PublicNode,
    // https://developers.cloudflare.com/web3/ethereum-gateway/
    Cloudflare,
}

impl EthereumProvider {
    fn ethereum_mainnet_endpoint_url(&self) -> &str {
        match self {
            EthereumProvider::Ankr => "https://rpc.ankr.com/eth",
            EthereumProvider::PublicNode => "https://ethereum.publicnode.com",
            EthereumProvider::Cloudflare => "https://cloudflare-eth.com",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum SepoliaProvider {
    // https://www.ankr.com/rpc/
    Ankr,
    // https://publicnode.com/
    PublicNode,
}

impl SepoliaProvider {
    fn ethereum_sepolia_endpoint_url(&self) -> &str {
        match self {
            SepoliaProvider::Ankr => "https://rpc.ankr.com/eth_sepolia",
            SepoliaProvider::PublicNode => "https://ethereum-sepolia.publicnode.com",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum AvalancheProvider {
    // https://www.ankr.com/rpc/
    Ankr,
    // https://publicnode.com/
    PublicNode,
}

impl AvalancheProvider {
    fn avalanche_endpoint_url(&self) -> &str {
        match self {
            AvalancheProvider::Ankr => "https://rpc.ankr.com/avalanche",
            AvalancheProvider::PublicNode => "https://avalanche-c-chain.publicnode.com",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum FujiProvider {
    // https://www.ankr.com/rpc/
    Ankr,
    // https://publicnode.com/
    PublicNode,
}

impl FujiProvider {
    fn avalanche_fuji_endpoint_url(&self) -> &str {
        match self {
            FujiProvider::Ankr => "https://rpc.ankr.com/avalanche_fuji",
            FujiProvider::PublicNode => "https://avalanche-fuji-c-chain.publicnode.com",
        }
    }
}
