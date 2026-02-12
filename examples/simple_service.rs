use binderbinder::{
    TransactionHandler,
    binder_object::BinderObjectOrRef,
    device::Transaction,
    payload::{BinderObjectType, PayloadBuilder},
};
use pion::*;
use std::{fs::File, path::Path, str::FromStr as _};
use tokio::task::spawn_blocking;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

const ECHO_CODE: u32 = 1;

pub struct EchoPort;
impl TransactionHandler for EchoPort {
    async fn handle(&self, mut transaction: Transaction) -> PayloadBuilder<'_> {
        let mut builder = PayloadBuilder::new();
        if transaction.code != ECHO_CODE {
            builder.push_bytes(b"unknown transaction code");
            return builder;
        }
        loop {
            let bytes = transaction.payload.bytes_until_next_obj();
            if bytes != 0 {
                let Ok(v) = transaction
                    .payload
                    .read_bytes(bytes)
                    .inspect_err(|err| error!("failed to read bytes: {err}"))
                else {
                    break;
                };
                builder.push_bytes(v);
                continue;
            }
            match transaction.payload.next_object_type() {
                Some(BinderObjectType::BinderRef)
                | Some(BinderObjectType::WeakBinderRef)
                | Some(BinderObjectType::BinderObject)
                | Some(BinderObjectType::WeakBinderObject) => {
                    builder.push_binder_ref(&transaction.payload.read_binder_ref().unwrap());
                    continue;
                }
                Some(BinderObjectType::Fd) => {
                    let (fd, cookie) = transaction.payload.read_fd().unwrap();
                    builder.push_owned_fd(fd, cookie);
                    continue;
                }
                _ => {}
            }
            break;
        }
        builder
    }

    async fn handle_one_way(&self, _transaction: binderbinder::device::Transaction) {
        info!("got oneway transaction")
    }
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or(EnvFilter::from_str("warn").unwrap()),
        )
        .with_thread_ids(true)
        .init();
    let dev = PionBinderDevice::new();

    let file_path = Path::new("/tmp/binder_echo_test.bind");
    let file = std::fs::File::create(file_path).unwrap();
    file.lock().unwrap();

    let echo_port = dev.register_object(EchoPort);
    dev.bind_binder_ref_to_file(file, BinderObjectOrRef::Object(echo_port))
        .await
        .unwrap();

    let port = dev
        .get_binder_ref_from_file(
            File::options()
                .write(true)
                .read(true)
                .open(file_path)
                .unwrap(),
        )
        .await
        .unwrap();
    let (_, mut res) = spawn_blocking(move || {
        let mut payload = PayloadBuilder::new();
        payload.push_bytes(b"Hello, world!");
        let Ok(v) = dev
            .transact_blocking(&port, ECHO_CODE, payload)
            .inspect_err(|err| error!("get error from transact_blocking: {err}"))
        else {
            loop {}
        };
        v
    })
    .await
    .unwrap();
    let res_bytes = res.read_bytes(res.bytes_until_next_obj()).unwrap();
    let string = String::from_utf8_lossy(res_bytes);
    info!("echo result: {string}");
    assert_eq!(string, "Hello, world!".to_string());
    tokio::signal::ctrl_c().await.unwrap();
}
