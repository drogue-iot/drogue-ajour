use anyhow::anyhow;

use drogue_ajour_protocol::{Command, Status};

use crate::hawkbit::HawkbitClient;
use crate::index::{FirmwareSpec, Index};
use crate::metadata::Metadata;
use crate::oci::OciClient;

pub struct Updater {
    index: Index,
    oci: Option<OciClient>,
    hawkbit: Option<HawkbitClient>,
}

impl Updater {
    pub fn new(index: Index, oci: Option<OciClient>, hawkbit: Option<HawkbitClient>) -> Self {
        Self {
            oci,
            index,
            hawkbit,
        }
    }
    pub async fn process(
        &mut self,
        application: &str,
        device: &str,
        status: Status,
    ) -> Result<Command, anyhow::Error> {
        if let Some(spec) = self.index.latest_version(application, device).await? {
            let index = &mut self.index;
            match spec {
                FirmwareSpec::OCI {
                    image,
                    image_pull_policy,
                } => {
                    if let Some(oci) = self.oci.as_mut() {
                        Self::process_update(
                            oci,
                            index,
                            application,
                            device,
                            status,
                            &(image.to_string(), image_pull_policy),
                        )
                        .await
                    } else {
                        let e = format!(
                            "Device {}/{} requested OCI firwmare, but no OCI registry configured",
                            application, device
                        );
                        log::warn!("{}", e);
                        Err(anyhow!("{}", e))
                    }
                }
                FirmwareSpec::HAWKBIT { controller } => {
                    if let Some(hb) = self.hawkbit.as_mut() {
                        hb.register(&controller).await?;
                        Self::process_update(hb, index, application, device, status, &controller)
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
            }
        } else {
            Err(anyhow!("Unable to find latest version for {}", application))
        }
    }

    async fn process_update<F>(
        store: &mut F,
        index: &mut Index,
        application: &str,
        device: &str,
        status: Status,
        params: &F::Params,
    ) -> Result<Command, anyhow::Error>
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
                    Ok(Command::new_sync(
                        &status.version,
                        None,
                        status.correlation_id,
                    ))
                } else {
                    let mut offset = 0;
                    let mut mtu = 512;
                    if let Some(m) = status.mtu {
                        mtu = m as usize;
                    }
                    if let Some(update) = status.update {
                        if update.version == metadata.version {
                            offset = update.offset as usize;
                        }
                    } else {
                        log::info!(
                            "Updating device {}/{} from {} to {}",
                            application,
                            device,
                            status.version,
                            metadata.version
                        );
                    }

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
                        ))
                    } else {
                        let data = hex::decode(&metadata.checksum)?;
                        Ok(Command::new_swap(
                            &metadata.version,
                            &data,
                            status.correlation_id,
                        ))
                    }
                }
            }
            Ok((ctx, None)) => {
                if let Err(e) = index
                    .update_status(
                        application,
                        device,
                        &status,
                        Err("Metadata not found".to_string()),
                    )
                    .await
                {
                    log::warn!(
                        "Error updating status of device {}/{}: {:?}",
                        application,
                        device,
                        e
                    );
                }
                Ok(Command::new_wait(
                    store.get_backoff(&ctx),
                    status.correlation_id,
                ))
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

    async fn mark_finished(
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
