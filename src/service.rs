use anyhow::Ok;
use rayon::prelude::ParallelSliceMut;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use spdlog::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

use crate::ExplorerEntry;

pub enum QueryResult {
    Success(String),
    PathNotFound,
    NotDirectory,
}

pub struct Service {
    address: SocketAddr,
    listener: TcpListener,
    directory: PathBuf,
}

impl Service {
    pub async fn new(address: SocketAddr, directory: PathBuf) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(address).await?;
        Ok(Self {
            address,
            listener,
            directory,
        })
    }

    pub async fn start_listening(&self) -> anyhow::Result<()> {
        info!("Server started at {}", self.address);
        loop {
            let (mut socket, _) = self.listener.accept().await?;
            let directory = self.directory.clone();
            tokio::spawn(async move { Self::handle_request(&mut socket, directory).await });
        }
    }

    async fn handle_request(
        socket: &mut TcpStream,
        directory: PathBuf,
    ) -> anyhow::Result<()> {
        let (_, mut writer) = socket.split();
        
        let make_http_response = |status: &str, content_type: &str, content: &str| -> String {
            format!(
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
                status,
                content_type,
                content.len(),
                content
            )
        };

        match Self::query_directory(directory.clone())? {
            QueryResult::Success(data_text) => {
                let response = make_http_response("200 OK", "application/json", &data_text);
                writer.write_all(response.as_bytes()).await?;
            }
            QueryResult::PathNotFound => {
                const MESSAGE: &str = "Path not found!";
                let response = make_http_response("404 Not Found", "text/plain", MESSAGE);
                warn!("{} {}", MESSAGE, directory.display());
                writer.write_all(response.as_bytes()).await?;
            }
            QueryResult::NotDirectory => {
                const MESSAGE: &str = "Not a directory!";
                let absolute_path = directory.canonicalize()?;
                warn!("{} {}", MESSAGE, absolute_path.display());
                let response = make_http_response("400 Bad Request", "text/plain", MESSAGE);
                writer.write_all(response.as_bytes()).await?;
            }
        }
        
        Ok(())
    }

    fn query_directory(directory: PathBuf) -> anyhow::Result<QueryResult> {
        if !directory.exists() {
            return Ok(QueryResult::PathNotFound);
        }
        if !directory.is_dir() {
            return Ok(QueryResult::NotDirectory);
        }

        let absolute_path = directory.canonicalize()?;
        let start_time = Instant::now();

        let mut file_list: Vec<ExplorerEntry> = std::fs::read_dir(&absolute_path)?
            .par_bridge()
            .map(|entry| ExplorerEntry::new(&entry.unwrap()))
            .collect::<Result<Vec<ExplorerEntry>, _>>()?;
        
        file_list.par_sort();

        let data_text = sonic_rs::to_string(&file_list)?;
        let elapsed = start_time.elapsed().as_micros() as f64 / 1000.0;

        debug!(
            "Response: {} items in {} tooks {}ms",
            file_list.len(),
            absolute_path.display(),
            elapsed
        );

        Ok(QueryResult::Success(data_text))
    }
}
