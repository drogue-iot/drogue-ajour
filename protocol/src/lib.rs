#![cfg_attr(not(feature = "std"), no_std)]
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Status<'a> {
    pub version: &'a str,
    pub mtu: Option<u32>,
    pub update: Option<UpdateStatus<'a>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateStatus<'a> {
    pub version: &'a str,
    pub offset: u32,
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
pub enum CommandRef<'a> {
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

impl<'a> CommandRef<'a> {
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

#[cfg(feature = "std")]
pub use owned::*;

#[cfg(feature = "std")]
mod owned {
    use super::CommandRef;
    use serde::{Deserialize, Serialize};

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

    impl<'a> From<CommandRef<'a>> for Command {
        fn from(r: CommandRef<'a>) -> Self {
            match r {
                CommandRef::Sync { version, poll } => Command::Sync {
                    version: version.to_string(),
                    poll,
                },
                CommandRef::Write {
                    version,
                    offset,
                    data,
                } => Command::Write {
                    version: version.to_string(),
                    offset,
                    data: data.to_vec(),
                },
                CommandRef::Swap { version, checksum } => Command::Swap {
                    version: version.to_string(),
                    checksum: checksum.to_vec(),
                },
            }
        }
    }
}
