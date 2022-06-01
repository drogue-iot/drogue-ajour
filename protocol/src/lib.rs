//! A protocol for updating firmware of embedded devices from a remote server. The protocol is not
//! tied to any specific platform, but is designed to work with Drogue Ajour and Drogue Cloud.
#![cfg_attr(not(feature = "std"), no_std)]

use serde::{Deserialize, Serialize};

pub type Sha256 = [u8; 32];

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusRef<'a> {
    pub version: &'a [u8],
    pub mtu: Option<u32>,
    pub correlation_id: Option<u32>,
    pub update: Option<UpdateStatusRef<'a>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateStatusRef<'a> {
    pub version: &'a [u8],
    pub offset: u32,
}

impl<'a> StatusRef<'a> {
    pub fn first(version: &'a [u8], mtu: Option<u32>, correlation_id: Option<u32>) -> Self {
        Self {
            version,
            mtu,
            correlation_id,
            update: None,
        }
    }

    pub fn update(
        version: &'a [u8],
        mtu: Option<u32>,
        offset: u32,
        next_version: &'a [u8],
        correlation_id: Option<u32>,
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
    Wait {
        correlation_id: Option<u32>,
        poll: Option<u32>,
    },
    Sync {
        #[serde(with = "serde_bytes")]
        version: &'a [u8],
        correlation_id: Option<u32>,
        poll: Option<u32>,
    },
    Write {
        #[serde(with = "serde_bytes")]
        version: &'a [u8],
        correlation_id: Option<u32>,
        offset: u32,
        #[serde(with = "serde_bytes")]
        data: &'a [u8],
    },
    Swap {
        #[serde(with = "serde_bytes")]
        version: &'a [u8],
        correlation_id: Option<u32>,
        checksum: Sha256,
    },
}

#[cfg(feature = "std")]
pub use owned::*;

#[cfg(feature = "std")]
mod owned {
    use std::fmt::Formatter;

    use super::{CommandRef, Sha256, StatusRef};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Command {
        Wait {
            correlation_id: Option<u32>,
            poll: Option<u32>,
        },
        Sync {
            #[serde(with = "serde_bytes")]
            version: Vec<u8>,
            correlation_id: Option<u32>,
            poll: Option<u32>,
        },
        Write {
            #[serde(with = "serde_bytes")]
            version: Vec<u8>,
            correlation_id: Option<u32>,
            offset: u32,
            #[serde(with = "serde_bytes")]
            data: Vec<u8>,
        },
        Swap {
            #[serde(with = "serde_bytes")]
            version: Vec<u8>,
            correlation_id: Option<u32>,
            checksum: Sha256,
        },
    }

    impl Command {
        pub fn new_wait(poll: Option<u32>, correlation_id: Option<u32>) -> Self {
            Self::Wait {
                correlation_id,
                poll,
            }
        }

        pub fn new_sync(version: &[u8], poll: Option<u32>, correlation_id: Option<u32>) -> Self {
            Self::Sync {
                version: version.to_vec(),
                correlation_id,
                poll,
            }
        }

        pub fn new_swap(version: &[u8], checksum: &[u8], correlation_id: Option<u32>) -> Self {
            let mut sha256 = [0; 32];
            let to_copy = core::cmp::min(sha256.len(), checksum.len());
            sha256[..to_copy].copy_from_slice(&checksum[..to_copy]);

            Self::Swap {
                version: version.to_vec(),
                correlation_id,
                checksum: sha256,
            }
        }

        pub fn new_write(
            version: &[u8],
            offset: u32,
            data: &[u8],
            correlation_id: Option<u32>,
        ) -> Self {
            Self::Write {
                version: version.to_vec(),
                correlation_id,
                offset,
                data: data.to_vec(),
            }
        }
    }

    impl<'a> From<CommandRef<'a>> for Command {
        fn from(r: CommandRef<'a>) -> Self {
            match r {
                CommandRef::Wait {
                    poll,
                    correlation_id,
                } => Command::Wait {
                    correlation_id,
                    poll,
                },
                CommandRef::Sync {
                    version,
                    poll,
                    correlation_id,
                } => Command::Sync {
                    version: version.to_vec(),
                    correlation_id,
                    poll,
                },
                CommandRef::Write {
                    version,
                    offset,
                    data,
                    correlation_id,
                } => Command::Write {
                    version: version.to_vec(),
                    correlation_id,
                    offset,
                    data: data.to_vec(),
                },
                CommandRef::Swap {
                    version,
                    correlation_id,
                    checksum,
                } => Command::Swap {
                    version: version.to_vec(),
                    correlation_id,
                    checksum,
                },
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Status {
        pub version: Vec<u8>,
        pub correlation_id: Option<u32>,
        pub mtu: Option<u32>,
        pub update: Option<UpdateStatus>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct UpdateStatus {
        pub version: Vec<u8>,
        pub offset: u32,
    }

    impl<'a> From<StatusRef<'a>> for Status {
        fn from(r: StatusRef<'a>) -> Self {
            Self {
                version: r.version.to_vec(),
                correlation_id: r.correlation_id,
                mtu: r.mtu,
                update: r.update.map(|u| UpdateStatus {
                    version: u.version.to_vec(),
                    offset: u.offset,
                }),
            }
        }
    }

    impl std::fmt::Display for Command {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
            match self {
                Self::Sync { .. } => write!(f, "Sync"),
                Self::Wait { .. } => write!(f, "Wait"),
                Self::Swap { .. } => write!(f, "Swap"),
                Self::Write { .. } => write!(f, "Write"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_ref() {
        let s = Command::new_write(b"1234", 0, &[1, 2, 3, 4], None);
        let out = serde_cbor::to_vec(&s).unwrap();

        let s: CommandRef = serde_cbor::from_slice(&out).unwrap();
        println!("Out: {:?}", s);
    }

    #[test]
    fn serialized_status_size() {
        // 1 byte version, 4 byte payload, 4 byte checksum
        let version = &[1];
        let mtu = Some(4);
        let cid = None;
        let offset = 0;
        let next_version = &[2];

        let s = StatusRef::first(version, mtu, cid);
        let first = encode(&s);

        let s = StatusRef::update(version, mtu, offset, next_version, cid);
        let update = encode(&s);
        println!(
            "Serialized size:\n FIRST:\t{}\nUPDATE:\t{}",
            first.len(),
            update.len(),
        );
    }

    #[test]
    fn serialized_command_size() {
        // 1 byte version, 4 byte payload, 4 byte checksum
        let version = &[1];
        let payload = &[1, 2, 3, 4];
        let checksum = &[1, 2, 3, 4];

        let s = Command::new_write(version, 0, payload, None);
        let write = encode(&s);

        let s = Command::new_wait(Some(1), None);
        let wait = encode(&s);

        let s = Command::new_sync(version, Some(1), None);
        let sync = encode(&s);

        let s = Command::new_swap(version, checksum, None);
        let swap = encode(&s);
        println!(
            "Serialized size:\n WRITE:\t{}\nWAIT:\t{}\nSYNC:\t{}\nSWAP:\t{}",
            write.len(),
            wait.len(),
            sync.len(),
            swap.len()
        );
    }

    fn encode<T>(value: &T) -> Vec<u8>
    where
        T: serde::Serialize,
    {
        serde_cbor::ser::to_vec_packed(value).unwrap()
    }
}
