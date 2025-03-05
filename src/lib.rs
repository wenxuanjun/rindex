mod explorer;
mod log;
mod service;

pub use explorer::ExplorerEntry;
pub use log::Log;
pub use service::{QueryResult, Service};

#[global_allocator]
static GLOBAL: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;
