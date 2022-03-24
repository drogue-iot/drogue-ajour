use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Status<'a> {
    version: &'a str,
    mtu: Option<u32>,
    update: Option<UpdateStatus<'a>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateStatus<'a> {
    version: &'a str,
    offset: u32,
}

impl<'a> Status<'a> {
    pub fn first(version: &'a str, mtu: Option<u32>) -> Self {
        Self {
            version,
            mtu,
            update: None,
        }
    }

    pub fn update(version: &'a str, mtu: Option<u32>, offset: u32, next_version: &'a str) -> Self {
        Self {
            version,
            mtu,
            update: Some(UpdateStatus {
                offset,
                version: next_version,
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command<'a> {
    Sync {
        version: &'a str,
        poll: Option<u32>,
    },
    Write {
        version: &'a str,
        offset: u32,
        data: &'a [u8],
    },
    Swap {
        version: &'a str,
        checksum: &'a [u8],
    },
}

impl<'a> Command<'a> {
    pub fn new_sync(version: &'a str, poll: Option<u32>) -> Self {
        Self::Sync { version, poll }
    }

    pub fn new_swap(version: &'a str, checksum: &'a [u8]) -> Self {
        Self::Swap { version, checksum }
    }

    pub fn new_write(version: &'a str, offset: u32, data: &'a [u8]) -> Self {
        Self::Write {
            version,
            offset,
            data,
        }
    }
}
