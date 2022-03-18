use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Status {
    pub version: String,
    pub mtu: Option<u32>,
    pub update: Option<UpdateStatus>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateStatus {
    pub version: String,
    pub offset: u32,
}

impl Status {
    pub fn first(version: &str, mtu: Option<u32>) -> Self {
        Self {
            version: version.to_string(),
            mtu,
            update: None,
        }
    }

    pub fn update(version: &str, mtu: Option<u32>, offset: u32, next_version: &str) -> Self {
        Self {
            version: version.to_string(),
            mtu,
            update: Some(UpdateStatus {
                offset,
                version: next_version.to_string(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    Sync {
        version: String,
        poll: Option<u32>,
    },
    Write {
        version: String,
        offset: u32,
        data: Vec<u8>,
    },
    Swap {
        version: String,
        checksum: Vec<u8>,
    },
}

impl Command {
    pub fn new_sync(version: &str, poll: Option<u32>) -> Self {
        Self::Sync {
            version: version.to_string(),
            poll,
        }
    }

    pub fn new_swap(version: &str, checksum: &[u8]) -> Self {
        Self::Swap {
            version: version.to_string(),
            checksum: checksum.into(),
        }
    }

    pub fn new_write(version: &str, offset: u32, data: &[u8]) -> Self {
        Self::Write {
            version: version.to_string(),
            offset,
            data: data.into(),
        }
    }
}
