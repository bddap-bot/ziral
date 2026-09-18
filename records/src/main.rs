use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use iroh::SecretKey;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Cli {
    #[arg(long, global = true, default_value = "ziral-records.key")]
    key_file: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long, default_value = "ziral-records")]
        directory: PathBuf,
    },
    Ban {
        id: String,
        #[arg(long, default_value = "ziral-records")]
        directory: PathBuf,
    },
    Identity,
}

async fn load_key(path: &Path) -> Result<SecretKey> {
    use std::io::Write;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
        && !parent.try_exists()?
    {
        ziral_records::private_directory(parent).await?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(mut file) => {
            let mut bytes = [0; 32];
            getrandom::getrandom(&mut bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::File::open(
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new(".")),
            )?
            .sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    ensure!(
        std::fs::metadata(path)?.permissions().mode() & 0o077 == 0,
        "key must be private"
    );
    let bytes: [u8; 32] = std::fs::read(path)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid key length"))?;
    Ok(SecretKey::from_bytes(&bytes))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { directory } => {
            let key = load_key(&cli.key_file).await?;
            let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
                .secret_key(key)
                .bind()
                .await?;
            tracing::info!(id = %endpoint.id(), "ziral records listening");
            let router = iroh::protocol::Router::builder(endpoint)
                .accept(
                    ziral_records::DOMAIN,
                    ziral_records::Receiver::new(directory),
                )
                .spawn();
            tokio::signal::ctrl_c().await?;
            router.shutdown().await?;
        }
        Command::Ban { id, directory } => ziral_records::ban(directory, &id).await?,
        Command::Identity => println!("{}", load_key(&cli.key_file).await?.public()),
    }
    Ok(())
}
