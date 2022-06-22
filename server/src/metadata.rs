use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metadata {
    #[serde(with = "serde_bytes")]
    pub version: Vec<u8>,
    pub checksum: String,
    pub size: u32,
}
