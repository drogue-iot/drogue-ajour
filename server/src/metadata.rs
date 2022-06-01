use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metadata {
    pub version: Vec<u8>,
    pub checksum: String,
    pub size: u32,
}
