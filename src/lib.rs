use std::{env, ffi::OsString, fs::File, ops::Deref, os::fd::OwnedFd, path::PathBuf, sync::Arc};

use binderbinder::{BinderDevice, binder_ports::BinderPort, payload::PayloadBuilder};
use tracing::error;

pub const REGISTER_CODE: u32 = 1;
pub const EXCHANGE_CODE: u32 = 2;

#[derive(Clone)]
pub struct PionBinderDevice {
    dev: Arc<BinderDevice>,
}

pub fn binder_device_path() -> PathBuf {
    PathBuf::from(
        env::var_os("PION_BINDER_DEVICE_PATH").unwrap_or(OsString::from("/dev/binderfs/binder")),
    )
}

impl PionBinderDevice {
    pub fn new() -> Self {
        let path = binder_device_path();
        let dev = BinderDevice::new(path).unwrap();
        Self { dev }
    }
    pub fn from_fd(fd: impl Into<OwnedFd>) -> Self {
        let dev = BinderDevice::from_fd(fd);
        Self { dev }
    }
    pub async fn bind_port_to_file(
        &self,
        file: File,
        port: BinderPort,
    ) -> binderbinder::error::Result<()> {
        let dev = self.dev.clone();
        tokio::task::spawn_blocking(move || {
            let mut builder = PayloadBuilder::new();
            builder.push_owned_fd(file.into(), 0);
            builder.push_port(&port);
            let (_, mut reply) = dev.transact_blocking(
                &BinderPort::Handle(dev.context_manager_handle()),
                REGISTER_CODE,
                builder,
            )?;
            let bytes = reply.bytes_until_next_obj();
            if bytes != 0 {
                let bytes = reply.read_bytes(bytes).unwrap();
                let str = String::from_utf8_lossy(bytes);
                error!("failed to bind port to file: {str}");
                return Err(binderbinder::Error::Unknown(0));
            }
            Ok(())
        })
        .await
        .unwrap()
    }
    pub async fn get_port_from_file(&self, file: File) -> binderbinder::error::Result<BinderPort> {
        let dev = self.dev.clone();
        tokio::task::spawn_blocking(move || {
            let mut builder = PayloadBuilder::new();
            builder.push_owned_fd(file.into(), 0);
            let (_, mut reply) = dev.transact_blocking(
                &BinderPort::Handle(dev.context_manager_handle()),
                EXCHANGE_CODE,
                builder,
            )?;
            match reply.read_port() {
                Ok(p) => Ok(p),
                Err(err) => {
                    error!("failed to read port from reply: {err}");
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
