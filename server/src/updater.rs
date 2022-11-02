use anyhow::anyhow;

use ajour_schema::*;
use embedded_update::{Command, Status};

use crate::file::FileClient;
use crate::hawkbit::HawkbitClient;
use crate::index::Index;
use crate::metadata::Metadata;
use crate::oci::OciClient;

pub struct Updater {
    index: Index,
    oci: Option<OciClient>,
    hawkbit: Option<HawkbitClient>,
    file: Option<FileClient>,
}

impl Updater {
    pub fn new(
        index: Index,
        oci: Option<OciClient>,
        hawkbit: Option<HawkbitClient>,
        file: Option<FileClient>,
    ) -> Self {
        Self {
            oci,
            index,
            hawkbit,
            file,
        }
    }
    pub async fn process<'a>(
        &mut self,
        application: &str,
        device: &str,
        status: &'a Status<'a>,
    ) -> Result<SerializedCommand, anyhow::Error> {
        if let Some(spec) = self.index.latest_version(application, device).await? {
            let index = &mut self.index;
            match spec {
                FirmwareSpec::OCI {
                    image,
                    image_pull_policy,
                    build: _,
                } => {
                    if let Some(oci) = self.oci.as_mut() {
                        Self::process_update(
                            oci,
                            index,
                            application,
                            device,
                            &status,
                            &(image.to_string(), image_pull_policy),
                        )
                        .await
                    } else {
                        let e = format!(
                            "Device {}/{} requested container firwmare, but no container registry configured",
                            application, device
                        );
                        log::warn!("{}", e);
                        Err(anyhow!("{}", e))
                    }
                }
                FirmwareSpec::HAWKBIT { controller } => {
                    if let Some(hb) = self.hawkbit.as_mut() {
                        hb.register(&controller).await?;
                        Self::process_update(hb, index, application, device, &status, &controller)
                            .await
                    } else {
                        let e = format!(
                            "Device {}/{} requested Hawkbit firwmare, but no Hawkbit configured",
                            application, device
                        );
                        log::warn!("{}", e);
                        Err(anyhow!("{}", e))
                    }
                }
                FirmwareSpec::FILE { name } => {
                    if let Some(f) = self.file.as_mut() {
                        Self::process_update(f, index, application, device, &status, &name).await
                    } else {
                        let e = format!(
                            "Device {}/{} requested firwmare from file, but no file registry configured",
                            application, device
                        );
                        log::warn!("{}", e);
                        Err(anyhow!("{}", e))
                    }
                }
            }
        } else {
            Err(anyhow!("Unable to find latest version for {}", application))
        }
    }

    async fn process_update<'a, F>(
        store: &mut F,
        index: &mut Index,
        application: &str,
        device: &str,
        status: &'a Status<'a>,
        params: &F::Params,
    ) -> Result<SerializedCommand, anyhow::Error>
    where
        F: FirmwareStore,
    {
        match store.fetch_metadata(params).await {
            Ok((ctx, Some(metadata))) => {
                // Update firmware status
                if let Err(e) = index
                    .update_status(application, device, &status, Ok(&metadata))
                    .await
                {
                    log::warn!(
                        "Error updating status of device {}/{}: {:?}",
                        application,
                        device,
                        e
                    );
                }

                log::debug!("Got metadata: {:?}", metadata);

                if status.version == metadata.version {
                    // Don't let this fail us
                    let _ = store.mark_synced(params, &ctx, true).await;
                    Ok(
                        Command::new_sync(&status.version.as_ref(), None, status.correlation_id)
                            .try_into()?,
                    )
                } else {
                    let mut offset = 0;
                    let mut mtu = 512;
                    if let Some(m) = status.mtu {
                        mtu = m as usize;
                    }
                    if let Some(update) = &status.update {
                        if update.version == metadata.version {
                            offset = update.offset as usize;
                        }
                    } else {
                        log::info!(
                            "Updating device {}/{} from {:?} to {:?}",
                            application,
                            device,
                            status.version,
                            metadata.version
                        );
                    }

                    let _ = store
                        .update_progress(params, &ctx, offset as u32, metadata.size)
                        .await;

                    if offset < metadata.size as usize {
                        let firmware = store.fetch_firmware(params, &ctx, &metadata).await?;

                        let to_copy = core::cmp::min(firmware.len() - offset, mtu);
                        let block = &firmware[offset..offset + to_copy];

                        log::trace!(
                            "Sending firmware block offset {} size {}",
                            offset,
                            block.len()
                        );
                        Ok(Command::new_write(
                            &metadata.version,
                            offset as u32,
                            block,
                            status.correlation_id,
                        )
                        .try_into()?)
                    } else {
                        let data = hex::decode(&metadata.checksum.trim_start_matches("sha256:"))
                            .map_err(|e| {
                                log::warn!("Error decoding hex: {:?}", e);
                                e
                            })?;
                        log::info!("Sending swap instruction back to device!");
                        Ok(
                            Command::new_swap(&metadata.version, &data, status.correlation_id)
                                .try_into()?,
                        )
                    }
                }
            }
            Ok((ctx, None)) => {
                // Don't update status, just ask device to wait
                Ok(Command::new_wait(store.get_backoff(&ctx), status.correlation_id).try_into()?)
            }
            Err(e) => {
                if let Err(e) = index
                    .update_status(application, device, &status, Err(e.to_string()))
                    .await
                {
                    log::warn!(
                        "Error updating status of device {}/{}: {:?}",
                        application,
                        device,
                        e
                    );
                }
                Err(e.into())
            }
        }
    }
}

#[derive(Debug)]
pub struct SerializedCommand {
    data: Vec<u8>,
}

impl SerializedCommand {
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..]
    }
}

impl<'a> TryFrom<Command<'a>> for SerializedCommand {
    type Error = serde_cbor::Error;
    fn try_from(command: Command<'a>) -> Result<Self, Self::Error> {
        let data = serde_cbor::ser::to_vec_packed(&command)?;
        Ok(Self { data })
    }
}

#[async_trait::async_trait]
pub trait FirmwareStore {
    type Params;

    async fn fetch_metadata(
        &mut self,
        params: &Self::Params,
    ) -> Result<(Self::Context, Option<Metadata>), anyhow::Error>;

    fn get_backoff(&self, _: &Self::Context) -> Option<u32> {
        None
    }

    async fn update_progress(
        &mut self,
        params: &Self::Params,
        context: &Self::Context,
        offset: u32,
        size: u32,
    ) -> Result<(), anyhow::Error>;

    async fn mark_synced(
        &mut self,
        params: &Self::Params,
        context: &Self::Context,
        success: bool,
    ) -> Result<(), anyhow::Error>;

    type Context;
    async fn fetch_firmware(
        &mut self,
        params: &Self::Params,
        context: &Self::Context,
        metadata: &Metadata,
    ) -> Result<Vec<u8>, anyhow::Error>;
}
