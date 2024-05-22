use anyhow::Result;
use rayon::prelude::ParallelSliceMut;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use snowboard::{headers, response, Request, Server};
use spdlog::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;

use crate::explorer::ExplorerError;
use crate::ExplorerEntry;

pub enum QueryResult {
    Success(String),
    PathNotFound,
    NotDirectory,
}

pub struct Service;

impl Service {
    pub fn new(address: SocketAddr, directory: PathBuf) -> Result<Self> {
        info!("Server started at {}", address);
        Server::new(address)?.run_async(move |req: Request| {
            let directory = directory.clone();
            Box::pin(async move {
                let full_path = directory.join(&req.url.to_string()[1..]);
                match Self::query_directory(full_path.clone()).unwrap() {
                    QueryResult::Success(data_text) => {
                        let headers = headers! { "Content-Type" => "application/json" };
                        response!(ok, data_text, headers)
                    }
                    QueryResult::PathNotFound => {
                        const MESSAGE: &str = "Path not found!";
                        warn!("{} {}", MESSAGE, full_path.display());
                        response!(not_found, MESSAGE)
                    }
                    QueryResult::NotDirectory => {
                        const MESSAGE: &str = "Not a directory!";
                        warn!("{} {}", MESSAGE, full_path.display());
                        response!(bad_request, MESSAGE)
                    }
                }
            })
        })
    }

    fn query_directory(full_path: PathBuf) -> Result<QueryResult> {
        if !full_path.exists() {
            return Ok(QueryResult::PathNotFound);
        }
        if !full_path.is_dir() {
            return Ok(QueryResult::NotDirectory);
        }

        let start_time = Instant::now();

        let mut file_list = std::fs::read_dir(&full_path)?
            .par_bridge()
            .filter_map(|entry| match ExplorerEntry::new(&entry.unwrap()) {
                Ok(explorer_entry) => Some(Ok(explorer_entry)),
                Err(err) => {
                    if let Some(ExplorerError::MissingSymlinkTarget(_)) =
                        err.downcast_ref::<ExplorerError>()
                    {
                        info!("{}", err);
                        None
                    } else {
                        Some(Err(err))
                    }
                }
            })
            .collect::<Result<Vec<ExplorerEntry>, _>>()?;

        file_list.par_sort();

        let data_text = sonic_rs::to_string(&file_list)?;
        let elapsed = start_time.elapsed().as_micros() as f64 / 1000.0;

        debug!(
            "Response: {} items in {} tooks {}ms",
            file_list.len(),
            full_path.display(),
            elapsed
        );

        Ok(QueryResult::Success(data_text))
    }
}
