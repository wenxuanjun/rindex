use anyhow::Result;
use argh::FromArgs;
use spdlog::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use rindex::{Log, Service};

static LOGGER: OnceLock<Arc<Logger>> = OnceLock::new();

#[derive(FromArgs)]
#[argh(description = "Fast Indexer compatible with nginx's autoindex module.")]
struct Args {
    #[argh(option, short = 'd')]
    #[argh(description = "base dir of the indexer")]
    directory: PathBuf,

    #[argh(option, short = 'a')]
    #[argh(default = "IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))")]
    #[argh(description = "ip address for listening")]
    address: IpAddr,

    #[argh(option, short = 'p')]
    #[argh(default = "3500")]
    #[argh(description = "port for listening")]
    port: u16,

    #[argh(option, short = 'f')]
    #[argh(description = "directory of log files, empty for disable")]
    logdir: Option<PathBuf>,

    #[argh(switch, short = 'v')]
    #[argh(description = "will show logs in stdout")]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Args = argh::from_env();
    LOGGER.get_or_init(|| Log::new(args.logdir, args.verbose));

    let address = SocketAddr::from((args.address, args.port));
    let server = Service::new(address, args.directory.canonicalize()?).await?;
    server.start_listening().await?;

    LOGGER.get().unwrap().flush();
    Ok(())
}
