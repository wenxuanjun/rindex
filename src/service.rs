use anyhow::{Ok, Result};
use rayon::prelude::ParallelSliceMut;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use spdlog::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
    pub async fn new(address: SocketAddr, directory: PathBuf) -> Result<Self> {
        let listener = TcpListener::bind(address).await?;
        Ok(Self {
            address,
            listener,
            directory,
        })
    }

    pub async fn start_listening(&self) -> Result<()> {
        info!("Server started at {}", self.address);
        loop {
            let (mut socket, _) = self.listener.accept().await?;
            let directory = self.directory.clone();
            tokio::spawn(async move { Self::handle_request(&mut socket, directory).await });
        }
    }

    async fn handle_request(socket: &mut TcpStream, directory: PathBuf) -> Result<()> {
        let (mut reader, mut writer) = socket.split();

        let make_http_response = |status: &str, content_type: &str, content: &str| -> String {
            format!(
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
                status,
                content_type,
                content.len(),
                content
            )
        };

        let full_path = {
            let mut buffer = [0; 1024];
            let bytes_read = reader.read(&mut buffer).await?;

            let request = String::from_utf8_lossy(&buffer[..bytes_read]);
            let request_line = request.lines().next().unwrap_or("");

            let request_parts: Vec<&str> = request_line.split_whitespace().collect();
            if request_parts.len() < 3 {
                return Err(anyhow::anyhow!("Invalid HTTP request"));
            }
            if request_parts[0] != "GET" {
                const MESSAGE: &str = "Method Not Allowed!";
                let response = make_http_response("404 Method Not Allowed", "text/plain", MESSAGE);
                writer.write_all(response.as_bytes()).await?;
                return Ok(());
            }
            directory.join(&request_parts[1][1..])
        };

        match Self::query_directory(full_path.clone())? {
            QueryResult::Success(data_text) => {
                let response = make_http_response("200 OK", "application/json", &data_text);
                writer.write_all(response.as_bytes()).await?;
            }
            QueryResult::PathNotFound => {
                const MESSAGE: &str = "Path not found!";
                warn!("{} {}", MESSAGE, full_path.display());
                let response = make_http_response("404 Not Found", "text/plain", MESSAGE);
                writer.write_all(response.as_bytes()).await?;
            }
            QueryResult::NotDirectory => {
                const MESSAGE: &str = "Not a directory!";
                warn!("{} {}", MESSAGE, full_path.display());
                let response = make_http_response("400 Bad Request", "text/plain", MESSAGE);
                writer.write_all(response.as_bytes()).await?;
            }
        }

        Ok(())
    }

    fn query_directory(directory: PathBuf) -> Result<QueryResult> {
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
