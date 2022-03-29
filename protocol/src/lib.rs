#![cfg_attr(not(feature = "std"), no_std)]

use serde::{Deserialize, Serialize};

pub type Sha256 = [u8; 32];

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusRef<'a> {
    pub version: &'a str,
    pub mtu: Option<u32>,
    pub correlation_id: Option<u32>,
    pub update: Option<UpdateStatusRef<'a>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateStatusRef<'a> {
    pub version: &'a str,
    pub offset: u32,
}

impl<'a> StatusRef<'a> {
    pub fn first(version: &'a str, correlation_id: Option<u32>, mtu: Option<u32>) -> Self {
        Self {
            version,
            mtu,
            correlation_id,
            update: None,
        }
    }

    pub fn update(
        version: &'a str,
        correlation_id: Option<u32>,
        mtu: Option<u32>,
        offset: u32,
        next_version: &'a str,
    ) -> Self {
        Self {
            version,
            mtu,
            correlation_id,
            update: Some(UpdateStatusRef {
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
        correlation_id: Option<u32>,
        poll: Option<u32>,
    },
    Write {
        version: &'a str,
        correlation_id: Option<u32>,
        offset: u32,
        #[serde(with = "serde_bytes")]
        data: &'a [u8],
    },
    Swap {
        version: &'a str,
        correlation_id: Option<u32>,
        checksum: Sha256,
    },
}

#[cfg(feature = "std")]
pub use owned::*;

#[cfg(feature = "std")]
mod owned {
    use super::{CommandRef, Sha256, StatusRef};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Command {
        Sync {
            version: String,
            correlation_id: Option<u32>,
            poll: Option<u32>,
        },
        Write {
            version: String,
            correlation_id: Option<u32>,
            offset: u32,
            #[serde(with = "serde_bytes")]
            data: Vec<u8>,
        },
        Swap {
            version: String,
            correlation_id: Option<u32>,
            checksum: Sha256,
        },
    }

    impl Command {
        pub fn new_sync(version: &str, poll: Option<u32>, correlation_id: Option<u32>) -> Self {
            Self::Sync {
                version: version.to_string(),
                correlation_id,
                poll,
            }
        }

        pub fn new_swap(version: &str, checksum: &[u8], correlation_id: Option<u32>) -> Self {
            let mut sha256 = [0; 32];
            sha256.copy_from_slice(&checksum[..32]);

            Self::Swap {
                version: version.to_string(),
                correlation_id,
                checksum: sha256,
            }
        }

        pub fn new_write(
            version: &str,
            offset: u32,
            data: &[u8],
            correlation_id: Option<u32>,
        ) -> Self {
            Self::Write {
                version: version.to_string(),
                correlation_id,
                offset,
                data: data.to_vec(),
            }
        }
    }

    impl<'a> From<CommandRef<'a>> for Command {
        fn from(r: CommandRef<'a>) -> Self {
            match r {
                CommandRef::Sync {
                    version,
                    poll,
                    correlation_id,
                } => Command::Sync {
                    version: version.to_string(),
                    correlation_id,
                    poll,
                },
                CommandRef::Write {
                    version,
                    offset,
                    data,
                    correlation_id,
                } => Command::Write {
                    version: version.to_string(),
                    correlation_id,
                    offset,
                    data: data.to_vec(),
                },
                CommandRef::Swap {
                    version,
                    correlation_id,
                    checksum,
                } => Command::Swap {
                    version: version.to_string(),
                    correlation_id,
                    checksum,
                },
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Status {
        pub version: String,
        pub correlation_id: Option<u32>,
        pub mtu: Option<u32>,
        pub update: Option<UpdateStatus>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct UpdateStatus {
        pub version: String,
        pub offset: u32,
    }

    impl<'a> From<StatusRef<'a>> for Status {
        fn from(r: StatusRef<'a>) -> Self {
            Self {
                version: r.version.to_string(),
                correlation_id: r.correlation_id,
                mtu: r.mtu,
                update: r.update.map(|u| UpdateStatus {
                    version: u.version.to_string(),
                    offset: u.offset,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_ref() {
        let s = Command::new_write("1234", 0, &[1, 2, 3, 4], None);
        let out = serde_cbor::to_vec(&s).unwrap();

        let s: CommandRef = serde_cbor::from_slice(&out).unwrap();
        println!("Out: {:?}", s);
    }
}
