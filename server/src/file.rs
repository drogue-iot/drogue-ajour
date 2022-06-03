use crate::metadata::Metadata;
use crate::updater::FirmwareStore;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

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
        let f = self.path.join(params).join(".json");
        let f: Metadata = serde_json::from_reader(File::open(f)?)?;
        Ok(((), Some(f)))
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
        let f = self.path.join(params).join(".bin");
        let mut f = File::open(f)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        Ok(data)
    }
}
