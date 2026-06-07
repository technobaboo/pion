use std::{env, ffi::OsString, fs::File, ops::Deref, os::fd::OwnedFd, path::PathBuf, sync::Arc};

use binderbinder::{
    BinderDevice,
    binder_object::{BinderObjectOrRef, ToBinderObjectOrRef},
    payload::PayloadBuilder,
};
use tracing::error;

pub const REGISTER_CODE: u32 = 1;
pub const EXCHANGE_CODE: u32 = 2;

#[derive(Debug, Clone)]
pub struct PionBinderDevice {
    dev: Arc<BinderDevice>,
}

pub fn binder_device_path() -> PathBuf {
    PathBuf::from(
        env::var_os("PION_BINDER_DEVICE_PATH").unwrap_or(OsString::from("/dev/pionfs/pion-binder")),
    )
}

impl Default for PionBinderDevice {
    fn default() -> Self {
        let path = binder_device_path();
        let dev = BinderDevice::new(path).unwrap();
        Self { dev }
    }
}

impl PionBinderDevice {
    pub fn from_fd(fd: impl Into<OwnedFd>, looper_threads: usize) -> Self {
        Self {
            dev: BinderDevice::from_fd(fd, looper_threads),
        }
    }
    pub async fn bind_binder_ref_to_file(
        &self,
        file: File,
        binder_ref: &impl ToBinderObjectOrRef,
    ) -> binderbinder::error::Result<()> {
        let dev = self.dev.clone();
        let binder_ref = binder_ref.to_binder_object_or_ref();
        tokio::task::spawn_blocking(move || {
            let mut builder = PayloadBuilder::new();
            builder.push_owned_fd(file.into(), 0);
            builder.push_binder_ref(&binder_ref);
            let (_, mut reply) =
                dev.transact_blocking(dev.context_manager(), REGISTER_CODE, builder)?;
            let bytes = reply.bytes_until_next_obj();
            if bytes != 0 {
                let bytes = reply.read_bytes(bytes).unwrap();
                let str = String::from_utf8_lossy(bytes);
                error!("failed to bind binder ref to file: {str}");
                return Err(binderbinder::Error::Unknown(1));
            }
            Ok(())
        })
        .await
        .unwrap()
    }
    pub async fn get_binder_ref_from_file(
        &self,
        file: File,
    ) -> binderbinder::error::Result<BinderObjectOrRef> {
        let dev = self.dev.clone();
        tokio::task::spawn_blocking(move || {
            let mut builder = PayloadBuilder::new();
            builder.push_owned_fd(file.into(), 0);
            let (_, mut reply) =
                dev.transact_blocking(dev.context_manager(), EXCHANGE_CODE, builder)?;
            match reply.read_binder_ref() {
                Ok(p) => Ok(p),
                Err(err) => {
                    error!("failed to read binder ref from reply: {err}");
                    let bytes = reply.bytes_until_next_obj();
                    if bytes != 0 {
                        let bytes = reply.read_bytes(bytes).unwrap();
                        let str = String::from_utf8_lossy(bytes);
                        error!("error msg from context manager: {str}");
                    }
                    Err(binderbinder::Error::Unknown(0))
                }
            }
        })
        .await
        .unwrap()
    }
    pub fn device(&self) -> &Arc<BinderDevice> {
        &self.dev
    }
}

impl Deref for PionBinderDevice {
    type Target = Arc<BinderDevice>;

    fn deref(&self) -> &Self::Target {
        self.device()
    }
}

impl PartialEq for PionBinderDevice {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(self.device(), other.device())
    }
}
