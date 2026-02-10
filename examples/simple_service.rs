use binderbinder::{
    TransactionHandler,
    binder_ports::BinderPort,
    device::Transaction,
    payload::{BinderObjectType, PayloadBuilder},
};
use pion::*;
use std::{fs::File, path::Path, str::FromStr as _, time::Duration};
use tokio::{task::spawn_blocking, time::sleep};
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
                Some(BinderObjectType::PortHandle)
                | Some(BinderObjectType::WeakPortHandle)
                | Some(BinderObjectType::WeakOwnedPort)
                | Some(BinderObjectType::OwnedPort) => {
                    builder.push_port(&transaction.payload.read_port().unwrap());
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
        .init();
    let dev = PionBinderDevice::new();

    sleep(Duration::from_secs(1)).await;
    let file_path = Path::new("/tmp/binder_echo_test.bind");
    let file = std::fs::File::create(file_path).unwrap();
    file.lock().unwrap();

    let echo_port = dev.register_object(EchoPort);
    dev.bind_port_to_file(file, BinderPort::Owned(echo_port))
        .await
        .unwrap();

    let port = dev
        .get_port_from_file(
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
        dev.transact_blocking(&port, ECHO_CODE, payload).unwrap()
    })
    .await
    .unwrap();
    let res_bytes = res.read_bytes(res.bytes_until_next_obj()).unwrap();
    let string = String::from_utf8_lossy(res_bytes);
    info!("echo result: {string}");
    assert_eq!(string, "Hello, world!".to_string());
}
