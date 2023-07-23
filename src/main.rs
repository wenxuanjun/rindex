use argh::FromArgs;
use std::fs::DirEntry;
use serde::Serialize;
use spdlog::prelude::*;
use spdlog::sink::{RotatingFileSink, RotationPolicy};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tiny_http::{Response, Server, StatusCode};
use rayon::prelude::{ParallelBridge, ParallelIterator};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

static LOGGER: OnceLock<Arc<Logger>> = OnceLock::new();

#[derive(FromArgs)]
#[argh(description = "Fast Indexer compatible with nginx's autoindex module.")]
struct Args {
    #[argh(option, short = 'd')]
    #[argh(description = "base dir of the indexer")]
    directory: String,

    #[argh(option, short = 'a')]
    #[argh(default = "IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))")]
    #[argh(description = "ip address for listening")]
    address: IpAddr,

    #[argh(option, short = 'p')]
    #[argh(default = "3000")]
    #[argh(description = "port for listening")]
    port: u16,

    #[argh(option, short = 't')]
    #[argh(default = "4")]
    #[argh(description = "number of threads of web server")]
    threads: usize,

    #[argh(option, short = 'f')]
    #[argh(description = "directory of log files, empty for disable")]
    logdir: Option<String>,

    #[argh(switch, short = 'v')]
    #[argh(description = "will show logs in stdout")]
    verbose: bool,
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
enum File {
    Directory {
        name: String,
        mtime: String,
    },
    File {
        name: String,
        size: u64,
        mtime: String,
    },
}

impl File {
    fn create(file: &DirEntry) -> File {
        let metadata = file.metadata().unwrap();
        let modified = metadata.modified().unwrap();
        let formatted = httpdate::fmt_http_date(modified);
        let name = String::from(file.file_name().to_str().unwrap());

        if metadata.is_dir() {
            File::Directory {
                name,
                mtime: formatted,
            }
        } else {
            File::File {
                name,
                size: metadata.len(),
                mtime: formatted,
            }
        }
    }
}

fn server_function(server: Arc<Server>, path: Arc<String>) {
    for request in server.incoming_requests() {
        let directory = String::from(&*path) + request.url();

        let response = match Path::new(&directory) {
            path if !path.exists() => {
                const MESSAGE: &str = "Path not found!";
                warn!("{} {}", MESSAGE, directory);
                Response::from_string(MESSAGE).with_status_code(StatusCode(404))
            }
            path if !path.is_dir() => {
                const MESSAGE: &str = "Not a directory!";
                let absolute_path = path.canonicalize().unwrap();
                warn!("{} {}", MESSAGE, absolute_path.display());
                Response::from_string(MESSAGE).with_status_code(StatusCode(400))
            }
            path => {
                let absolute_path = path.canonicalize().unwrap();
                let start_time = Instant::now();
                let file_list: Vec<File> = std::fs::read_dir(&absolute_path)
                    .unwrap()
                    .par_bridge()
                    .map(|entry| File::create(&entry.unwrap()))
                    .collect();
                let data_text = simd_json::to_string(&file_list).unwrap();
                let elapsed = start_time.elapsed().as_micros() as f64 / 1000.0;

                debug!(
                    "Response: {} items in {} tooks {}ms",
                    file_list.len(),
                    absolute_path.display().to_string(),
                    elapsed
                );
                Response::from_string(data_text)
            }
        };

        request.respond(response).unwrap();
        LOGGER.get().unwrap().flush();
    }
}

fn init_logger(args: &Args) -> Arc<Logger> {
    let mut logger: LoggerBuilder = Logger::builder();
    let sinks = spdlog::default_logger().sinks().to_owned();

    let filter = if args.verbose {
        LevelFilter::MoreSevereEqual(Level::Debug)
    } else {
        LevelFilter::MoreSevereEqual(Level::Info)
    };

    logger.sinks(sinks).level_filter(filter);

    if let Some(path) = &args.logdir {
        let path = PathBuf::from(path);

        if !path.exists() && !path.is_dir() {
            panic!("Invalid log directory: {}", path.display());
        }

        let log_name = format!("{}.log", env!("CARGO_PKG_NAME"));
        let path = PathBuf::from(path).join(log_name);

        let file_sink: Arc<RotatingFileSink> = Arc::new(
            RotatingFileSink::builder()
                .base_path(path)
                .rotation_policy(RotationPolicy::Daily { hour: 0, minute: 0 })
                .rotate_on_open(false)
                .build()
                .unwrap(),
        );

        logger.sink(file_sink);
    }

    let logger = Arc::new(logger.build().unwrap());
    spdlog::swap_default_logger(logger.clone());

    return logger;
}

fn main() {
    let args: Args = argh::from_env();
    LOGGER.get_or_init(|| init_logger(&args));

    let address = SocketAddr::from((args.address, args.port));
    let server = Arc::new(Server::http(address).unwrap());
    let mut guards = Vec::with_capacity(args.threads);
    let directory = Arc::new(args.directory);

    for _ in 0..args.threads {
        let server = server.clone();
        let directory = directory.clone();
        let guard = std::thread::spawn(move || {
            server_function(server, directory);
        });
        guards.push(guard);
    }

    info!(
        "Server started at {} with {} threads",
        address, args.threads
    );
    LOGGER.get().unwrap().flush();

    guards.into_iter().for_each(|guard| guard.join().unwrap());
}
