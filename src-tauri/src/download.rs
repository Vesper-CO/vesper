//! Parallel, resumable, hash-verified downloads.
//!
//! - Files are streamed to `<name>.part`; if a download is interrupted, the next attempt sends a
//!   `Range` request and continues where it stopped.
//! - A file is only moved into place after its SHA-1 matches (when a hash is known).
//! - Existing files are re-verified, so a corrupted file is silently repaired.
use anyhow::{bail, Context, Result};
use futures_util::{stream, StreamExt, TryStreamExt};
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use sha1::{Digest, Sha1};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::{fs, io::AsyncWriteExt};

static CLIENT: OnceLock<Client> = OnceLock::new();

/// Shared HTTP client. Modrinth requires a descriptive User-Agent.
pub fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("VesperLauncher/", env!("CARGO_PKG_VERSION"), " (github.com/jakedev2/vesper)"))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()
            .expect("http client")
    })
}

pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let r = client().get(url).send().await.with_context(|| format!("Request failed: {url}"))?;
    let r = r.error_for_status().with_context(|| format!("Bad response from {url}"))?;
    Ok(r.json().await.with_context(|| format!("Invalid JSON from {url}"))?)
}

#[derive(Clone, Debug)]
pub struct Job {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

pub async fn sha1_file(path: &Path) -> Result<String> {
    let p = path.to_owned();
    tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;
        let mut f = std::fs::File::open(p)?;
        let mut h = Sha1::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        Ok(hex::encode(h.finalize()))
    })
    .await?
}

/// Is the file already on disk and correct?
pub async fn is_valid(job: &Job) -> bool {
    let Ok(meta) = fs::metadata(&job.path).await else { return false };
    if let Some(s) = job.size {
        if meta.len() != s {
            return false;
        }
    }
    match &job.sha1 {
        Some(h) => sha1_file(&job.path).await.map(|x| x.eq_ignore_ascii_case(h)).unwrap_or(false),
        None => true,
    }
}

fn part_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

async fn fetch_once(job: &Job) -> Result<()> {
    if let Some(parent) = job.path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let part = part_path(&job.path);
    let start = fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    let mut req = client().get(&job.url);
    if start > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={start}-"));
    }
    let resp = req.send().await?;
    let status = resp.status();
    if status.as_u16() == 416 {
        let _ = fs::remove_file(&part).await; // stale partial file: start over on retry
        bail!("partial file rejected, restarting");
    }
    if !status.is_success() {
        bail!("HTTP {} for {}", status, job.url);
    }
    let mut file = if status.as_u16() == 206 && start > 0 {
        fs::OpenOptions::new().append(true).open(&part).await?
    } else {
        fs::File::create(&part).await?
    };
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        file.write_all(&chunk?).await?;
    }
    file.flush().await?;
    drop(file);
    if let Some(h) = &job.sha1 {
        let got = sha1_file(&part).await?;
        if !got.eq_ignore_ascii_case(h) {
            let _ = fs::remove_file(&part).await;
            bail!("Hash mismatch for {}", job.url);
        }
    }
    fs::rename(&part, &job.path).await?;
    Ok(())
}

/// Download one file with up to 3 attempts.
pub async fn download_one(job: &Job) -> Result<()> {
    let mut last = None;
    for attempt in 1..=3u64 {
        match fetch_once(job).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(Duration::from_millis(400 * attempt)).await;
            }
        }
    }
    Err(last.unwrap()).with_context(|| format!("Could not download {}", job.url))
}

#[derive(Serialize, Clone)]
struct Progress<'a> {
    label: &'a str,
    done: usize,
    total: usize,
}

/// Download many files, 16 at a time, skipping the ones that are already valid.
pub async fn download_all(app: &AppHandle, label: &str, jobs: Vec<Job>) -> Result<()> {
    let total = jobs.len();
    if total == 0 {
        return Ok(());
    }
    let done = Arc::new(AtomicUsize::new(0));
    let _ = app.emit("progress", Progress { label, done: 0, total });
    stream::iter(jobs)
        .map(|job| {
            let app = app.clone();
            let done = done.clone();
            let label = label.to_string();
            async move {
                if !is_valid(&job).await {
                    download_one(&job).await?;
                }
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                if n % 6 == 0 || n == total {
                    let _ = app.emit("progress", Progress { label: &label, done: n, total });
                }
                Ok::<(), anyhow::Error>(())
            }
        })
        .buffer_unordered(16)
        .try_collect::<Vec<()>>()
        .await?;
    Ok(())
}
