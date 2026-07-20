use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWrappedEpochLocator {
    pub key_epoch: u64,
    pub wrapping_key_id: String,
}

impl RemoteWrappedEpochLocator {
    pub fn new(
        key_epoch: u64,
        wrapping_key_id: impl Into<String>,
    ) -> Result<Self, SyncRemoteError> {
        if key_epoch == 0 {
            return invalid_request("wrapped epoch locator key_epoch must be greater than zero");
        }
        let wrapping_key_id = wrapping_key_id.into();
        validate_query_component("wrapping_key_id", &wrapping_key_id, true)?;
        Ok(Self {
            key_epoch,
            wrapping_key_id,
        })
    }
}

pub struct RemoteWrappedEpochMaterialSource<'a, T> {
    client: &'a SyncRemoteClient<T>,
    locators: Vec<RemoteWrappedEpochLocator>,
}

impl<'a, T> RemoteWrappedEpochMaterialSource<'a, T> {
    pub fn new(
        client: &'a SyncRemoteClient<T>,
        mut locators: Vec<RemoteWrappedEpochLocator>,
    ) -> Result<Self, SyncRemoteError> {
        for locator in &locators {
            if locator.key_epoch == 0 {
                return invalid_request(
                    "wrapped epoch locator key_epoch must be greater than zero",
                );
            }
            validate_query_component("wrapping_key_id", &locator.wrapping_key_id, true)?;
        }
        locators.sort_by(|left, right| {
            left.key_epoch
                .cmp(&right.key_epoch)
                .then_with(|| left.wrapping_key_id.cmp(&right.wrapping_key_id))
        });
        if locators.windows(2).any(|pair| pair[0] == pair[1]) {
            return invalid_request("wrapped epoch locators cannot contain duplicates");
        }
        Ok(Self { client, locators })
    }
}

impl<T> fmt::Debug for RemoteWrappedEpochMaterialSource<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteWrappedEpochMaterialSource")
            .field("locator_count", &self.locators.len())
            .finish()
    }
}

impl<T: SyncRemoteTransport> SyncWrappedEpochMaterialSource
    for RemoteWrappedEpochMaterialSource<'_, T>
{
    fn load_wrapped_epoch_materials(
        &mut self,
        domain_id: &str,
        local_device_id: &str,
    ) -> Result<Vec<WrappedEpochMaterial>, SyncCryptoLoadError> {
        self.locators
            .iter()
            .map(|locator| {
                self.client
                    .device_wrapped_epoch_material(
                        domain_id,
                        local_device_id,
                        locator.key_epoch,
                        &locator.wrapping_key_id,
                    )
                    .map_err(map_wrapped_epoch_remote_error)
            })
            .collect()
    }
}
