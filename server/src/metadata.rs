use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metadata {
    pub version: String,
    pub checksum: String,
    pub size: u32,
}
