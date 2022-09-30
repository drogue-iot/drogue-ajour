use crate::metadata::Metadata;
use crate::updater::FirmwareStore;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    pub version: String,
    pub checksum: String,
    pub size: u32,
}

impl Into<Metadata> for FileMetadata {
    fn into(self) -> Metadata {
        Metadata {
            version: self.version.as_bytes().to_vec(),
            checksum: self.checksum,
            size: self.size,
        }
    }
}

pub struct FileClient {
    path: PathBuf,
}

impl FileClient {
    pub fn new(path: &PathBuf) -> Self {
        Self { path: path.clone() }
    }
}

#[async_trait::async_trait]
impl FirmwareStore for FileClient {
    type Params = String;

    async fn fetch_metadata(
        &mut self,
        params: &Self::Params,
    ) -> Result<(Self::Context, Option<Metadata>), anyhow::Error> {
        let f = self.path.join(format!("{}.json", params));
        log::debug!("Looking for metadata from {:?}", f);
        let f: FileMetadata = serde_json::from_reader(File::open(f)?)?;
        Ok(((), Some(f.try_into()?)))
    }

    async fn update_progress(
        &mut self,
        _: &Self::Params,
        _: &Self::Context,
        _: u32,
        _: u32,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn mark_synced(
        &mut self,
        _: &Self::Params,
        _: &Self::Context,
        _: bool,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    type Context = ();
    async fn fetch_firmware(
        &mut self,
        params: &Self::Params,
        _: &Self::Context,
        _: &Metadata,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let f = self.path.join(format!("{}.bin", params));
        log::debug!("Reading firmware from from {:?}", f);
        let mut f = File::open(f)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        Ok(data)
    }
}
