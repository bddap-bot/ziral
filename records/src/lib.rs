use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::sync::Mutex;

pub const DOMAIN: &[u8] = b"ziral-record/1";
const RECORD_LIMIT: usize = 32 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 1024 * 1024 * 1024;
const SESSION_LIMIT: usize = 1024;

#[derive(Debug, Deserialize)]
pub struct Batch {
    pub session: String,
    pub build: String,
    pub seed: u64,
    pub start: usize,
    pub inputs: Vec<Value>,
}

#[derive(Deserialize, Serialize)]
struct Record {
    session: String,
    build: String,
    seed: u64,
    inputs: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct Receiver {
    directory: PathBuf,
    write: Arc<Mutex<()>>,
}

impl Receiver {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            write: Arc::new(Mutex::new(())),
        }
    }

    pub async fn store(&self, batch: Batch) -> Result<usize> {
        ensure!(valid_id(&batch.session), "invalid session");
        ensure!(
            batch.build.len() == 40 && batch.build.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid build"
        );
        ensure!(
            batch.seed == 0 && !batch.inputs.is_empty() && batch.inputs.len() <= 4096,
            "invalid batch"
        );
        ensure!(
            !self
                .directory
                .join(format!("{}.revoked", batch.session))
                .try_exists()?,
            "session banned"
        );
        let _write = self
            .write
            .try_lock()
            .map_err(|_| anyhow::anyhow!("receiver busy"))?;
        let (existing, mut live) = usage(&self.directory, &batch.session).await?;
        let path =
            existing.unwrap_or_else(|| self.directory.join(format!("{}.json", batch.session)));
        let mut record = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice::<Record>(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Record {
                session: batch.session.clone(),
                build: batch.build.clone(),
                seed: batch.seed,
                inputs: Vec::new(),
            },
            Err(error) => return Err(error.into()),
        };
        ensure!(
            record.session == batch.session
                && record.build == batch.build
                && record.seed == batch.seed,
            "session changed"
        );
        ensure!(batch.start <= record.inputs.len(), "missing prefix");
        for (offset, input) in batch.inputs.into_iter().enumerate() {
            if let Some(existing) = record.inputs.get(batch.start + offset) {
                ensure!(existing == &input, "retry changed input");
            } else {
                record.inputs.push(input);
            }
        }
        let bytes = serde_json::to_vec(&record)?;
        ensure!(bytes.len() <= RECORD_LIMIT, "record full");
        live.sort();
        let mut total = live.iter().map(|session| session.1).sum::<u64>();
        if path.parent() == Some(&self.directory) {
            total += bytes.len() as u64;
        }
        let mut evicted = 0;
        while live.len() - evicted >= SESSION_LIMIT || total > TOTAL_LIMIT {
            let (_, length, oldest) = &live[evicted];
            match tokio::fs::remove_file(oldest).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            total -= length;
            evicted += 1;
        }
        private_directory(&self.directory).await?;
        if evicted > 0 {
            sync_directory(&self.directory).await?;
        }
        let parent = path.parent().unwrap();
        private_directory(parent).await?;
        let temporary = path.with_extension("pending");
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        let mut file = options.open(&temporary).await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &bytes).await?;
        tokio::io::AsyncWriteExt::flush(&mut file).await?;
        file.sync_all().await?;
        tokio::fs::rename(temporary, &path).await?;
        sync_directory(parent).await?;
        Ok(record.inputs.len())
    }
}

pub async fn private_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let mut created = Vec::new();
    for directory in path.ancestors() {
        if directory.as_os_str().is_empty() || directory.try_exists()? {
            break;
        }
        created.push(directory);
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    for directory in created {
        sync_directory(directory.parent().unwrap_or(Path::new("."))).await?;
    }
    Ok(())
}

async fn sync_directory(directory: &Path) -> Result<()> {
    let directory = if directory.as_os_str().is_empty() {
        Path::new(".")
    } else {
        directory
    };
    tokio::fs::File::open(directory).await?.sync_all().await?;
    Ok(())
}

fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')
}

pub async fn ban(directory: PathBuf, id: &str) -> Result<()> {
    ensure!(valid_id(id), "invalid session");
    private_directory(&directory).await?;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false).mode(0o600);
    options
        .open(directory.join(format!("{id}.revoked")))
        .await?
        .sync_all()
        .await?;
    sync_directory(&directory).await
}

// Subdirectories are archives and banned sessions are held for review: an upload
// flood can neither evict them nor fill the bounds with them.
async fn usage(
    root: &Path,
    session: &str,
) -> Result<(Option<PathBuf>, Vec<(SystemTime, u64, PathBuf)>)> {
    let mut pending = vec![root.to_owned()];
    let mut existing = None;
    let mut live = Vec::new();
    let mut banned = std::collections::HashSet::new();
    let name = format!("{session}.json");
    while let Some(directory) = pending.pop() {
        let mut entries = match tokio::fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let kind = entry.file_type().await?;
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "pending") {
                tokio::fs::remove_file(path).await?;
            } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if entry.file_name() == name.as_str() {
                    ensure!(existing.is_none(), "duplicate session files");
                    existing = Some(path);
                } else if directory == root {
                    let metadata = entry.metadata().await?;
                    live.push((metadata.modified()?, metadata.len(), path));
                }
            } else if directory == root && path.extension().is_some_and(|ext| ext == "revoked") {
                banned.insert(path.with_extension("json"));
            }
        }
    }
    live.retain(|session| !banned.contains(&session.2));
    Ok((existing, live))
}

impl iroh::protocol::ProtocolHandler for Receiver {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> std::result::Result<(), iroh::protocol::AcceptError> {
        let result: Result<()> = async {
            let (mut send, mut recv) =
                tokio::time::timeout(Duration::from_secs(15), connection.accept_bi()).await??;
            let result: Result<usize> = async {
                let mut length = [0; 4];
                tokio::time::timeout(Duration::from_secs(15), recv.read_exact(&mut length))
                    .await??;
                let length = u32::from_le_bytes(length) as usize;
                ensure!(length <= 1_000_000, "batch too large");
                let mut bytes = vec![0; length];
                tokio::time::timeout(Duration::from_secs(15), recv.read_exact(&mut bytes))
                    .await??;
                let batch = serde_json::from_slice(&bytes)
                    .map_err(|_| anyhow::anyhow!("invalid batch JSON"))?;
                self.store(batch).await
            }
            .await;
            let response = match result {
                Ok(next) => serde_json::json!({"next": next}),
                Err(error) => {
                    tracing::info!(reason = %error, "record upload rejected");
                    serde_json::json!({"error": "record upload rejected"})
                }
            };
            let bytes = serde_json::to_vec(&response)?;
            tokio::time::timeout(Duration::from_secs(15), async {
                send.write_all(&(bytes.len() as u32).to_le_bytes()).await?;
                send.write_all(&bytes).await?;
                send.finish()?;
                send.stopped().await?;
                anyhow::Ok(())
            })
            .await??;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            tracing::info!(reason = %error, "record connection rejected");
        }
        Ok(())
    }
}
