use binderbinder::{
    TransactionHandler, binder_ports::BinderPort, device::Transaction, fs::Binderfs,
    payload::PayloadBuilder,
};
use dashmap::{DashMap, Entry};
use pion::{EXCHANGE_CODE, PionBinderDevice, REGISTER_CODE, binder_device_path};
use std::{
    fs::File,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::{MetadataExt, PermissionsExt},
    },
    str::FromStr,
};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Default)]
struct Pion(DashMap<(u64, u64), BinderPort>);
impl Pion {
    fn entry<'a>(&'a self, fd: BorrowedFd<'_>) -> Option<Entry<'a, (u64, u64), BinderPort>> {
        let file: File = fd.try_clone_to_owned().ok()?.into();

        let metadata = file.metadata().ok()?;

        if metadata.permissions().readonly() {
            error!("permission denied");
            return None;
        }

        Some(self.0.entry((metadata.dev(), metadata.ino())))
    }
}
impl TransactionHandler for Pion {
    async fn handle(&self, mut transaction: Transaction) -> PayloadBuilder<'_> {
        let mut builder = PayloadBuilder::new();
        match transaction.code {
            REGISTER_CODE => {
                let fd = transaction.payload.read_fd();
                let port = transaction.payload.read_port();
                if let (Ok((fd, _)), Ok(handle)) = (fd, port)
                    && let Some(entry) = self.entry(fd.as_fd())
                {
                    match entry {
                        Entry::Occupied(_) => {
                            builder.push_bytes(c"couldn't register object".to_bytes_with_nul());
                            warn!("Tried to register Port on existing Fd");
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(handle);
                            info!("Registered Port");
                        }
                    }
                }
            }
            EXCHANGE_CODE => {
                let fd = transaction.payload.read_fd();
                if let Ok((fd, _)) = fd
                    && let Some(entry) = self.entry(fd.as_fd())
                {
                    match entry {
                        Entry::Occupied(entry) => {
                            let port = entry.get();
                            builder.push_port(port);
                        }
                        Entry::Vacant(_) => {
                            builder.push_bytes(c"couldn't find object".to_bytes_with_nul());
                            warn!("Failed to get object, not registered");
                        }
                    }
                }
            }
            _ => {
                builder.push_bytes(c"unkown transaction code".to_bytes_with_nul());
            }
        };
        builder
    }

    async fn handle_one_way(&self, _transaction: Transaction) {
        info!("got oneway transaction?");
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or(EnvFilter::from_str("debug").unwrap()),
        )
        .init();

    // let device_path = binder_device_path();
    // let binderfs_path = device_path.parent().unwrap();
    //
    // let binder_fs = Binderfs::mount(binderfs_path).unwrap();
    //
    // let _ = std::fs::remove_file(&device_path);
    //
    // let device_fd = binder_fs
    //     .create_device(device_path.file_name().unwrap())
    //     .unwrap();

    info!("Creating BinderDevice");
    // let device = PionBinderDevice::from_fd(device_fd);
    let device = PionBinderDevice::new();

    let port = device.register_object(Pion(DashMap::new()));
    device
        .set_context_manager(&port)
        .await
        .expect("failed to set context manager");
    info!("set context manager?");

    // let mut perms = std::fs::metadata(&device_path)
    //     .expect("IO error")
    //     .permissions();
    // perms.set_mode(0o666);
    //
    // std::fs::set_permissions(&device_path, perms).expect("Couldn't set permissions");

    // let _ = symlink(device_path, Path::new("/dev/binder"));
    tokio::signal::ctrl_c().await.unwrap()
}
