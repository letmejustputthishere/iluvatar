use candid::CandidType;
use minicbor_derive::{Decode, Encode};
use serde::Deserialize;

type HeaderField = (String, String);
type Headers = Vec<HeaderField>;
type Bytes = Vec<u8>;

/// An asset to be served via HTTP requests.
#[derive(CandidType, Clone, Deserialize, PartialEq, Debug, Encode, Decode)]
pub struct Asset {
    #[n(0)]
    pub headers: Headers,
    #[n(1)]
    pub bytes: Bytes,
}
