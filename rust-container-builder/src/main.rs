use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod dockerfile;
mod storage;
mod engine;
mod registry_client;

use engine::BuildEngine;
use storage::StorageManager;
use registry_client::RegistryClient;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
enum Args {
    /// Build a container image from a Dockerfile
    Build(BuildArgs),

    /// Push a built image to a registry
    Push(PushArgs),

    /// Pull an image from a registry
    Pull(PullArgs),
}

#[derive(clap::Args)]
struct BuildArgs {
    /// Path to the build context
    #[arg(short, long, default_value = ".")]
    context: PathBuf,

    /// Path to the Dockerfile
    #[arg(short, long, default_value = "./Dockerfile")]
    dockerfile: PathBuf,

    /// Name of the output image
    #[arg(short, long)]
    image_name: String,

    /// Output directory for build artifacts
    #[arg(long, default_value = "./build-output")]
    output_dir: PathBuf,

    /// Verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(clap::Args)]
struct PushArgs {
    /// Name of the image to push (including registry URL)
    #[arg(short, long)]
    image_name: String,

    /// Path to the build context (used to rebuild if needed)
    #[arg(short, long, default_value = ".")]
    context: PathBuf,

    /// Path to the Dockerfile (used to rebuild if needed)
    #[arg(short, long, default_value = "./Dockerfile")]
    dockerfile: PathBuf,

    /// Output directory for build artifacts
    #[arg(long, default_value = "./build-output")]
    output_dir: PathBuf,

    /// Verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(clap::Args)]
struct PullArgs {
    /// Name of the image to pull (including registry URL)
    #[arg(short, long)]
    image_name: String,

    /// Output directory for pulled image artifacts
    #[arg(long, default_value = "./pull-output")]
    output_dir: PathBuf,

    /// Verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    match Args::parse() {
        Args::Build(args) => build_command(args).await,
        Args::Push(args) => push_command(args).await,
        Args::Pull(args) => pull_command(args).await,
    }
}

async fn build_command(args: BuildArgs) -> Result<()> {
    // Initialize tracing
    if args.verbose > 0 {
        let level = match args.verbose {
            1 => "info",
            2 => "debug",
            _ => "trace",
        };
        unsafe {
            std::env::set_var("RUST_LOG", level);
        }
    }

    tracing_subscriber::fmt::init();

    tracing::info!("Starting Rust container builder");
    tracing::info!("Context: {:?}", args.context);
    tracing::info!("Dockerfile: {:?}", args.dockerfile);
    tracing::info!("Image name: {}", args.image_name);

    // Initialize storage manager
    let storage = StorageManager::new(args.output_dir)?;
    storage.init().await?;

    // Create build engine
    let mut engine = BuildEngine::new(storage, args.context);

    // Build the image
    let image = engine.build_image(&args.dockerfile, &args.image_name).await?;

    tracing::info!("Successfully built image: {}", image.name);
    tracing::info!("Image ID: {}", image.id);
    tracing::info!("Number of layers: {}", image.layers.len());

    Ok(())
}

async fn push_command(args: PushArgs) -> Result<()> {
    // Initialize tracing
    if args.verbose > 0 {
        let level = match args.verbose {
            1 => "info",
            2 => "debug",
            _ => "trace",
        };
        unsafe {
            std::env::set_var("RUST_LOG", level);
        }
    }

    tracing_subscriber::fmt::init();

    tracing::info!("Starting push operation");
    tracing::info!("Image name: {}", args.image_name);

    // Extract registry URL from image name
    let registry_url = extract_registry_url(&args.image_name);
    tracing::info!("Target registry: {}", registry_url);

    // Initialize storage manager
    let storage = StorageManager::new(args.output_dir)?;
    storage.init().await?;

    // Check if image exists in storage, if not build it
    let image = if let Some(stored_image) = storage.get_image_by_name(&args.image_name).await? {
        tracing::info!("Found existing image in storage, using it for push");
        stored_image
    } else {
        tracing::info!("Image not found in storage, building it first");
        let mut engine = BuildEngine::new(storage.clone_for_build(), args.context);
        engine.build_image(&args.dockerfile, &args.image_name).await?
    };

    // Create registry client
    let client = RegistryClient::new(registry_url)?;

    // Push the image
    client.push_image(&args.image_name, &image).await?;

    tracing::info!("Successfully pushed image: {}", args.image_name);
    Ok(())
}

async fn pull_command(args: PullArgs) -> Result<()> {
    // Initialize tracing
    if args.verbose > 0 {
        let level = match args.verbose {
            1 => "info",
            2 => "debug",
            _ => "trace",
        };
        unsafe {
            std::env::set_var("RUST_LOG", level);
        }
    }

    tracing_subscriber::fmt::init();

    tracing::info!("Starting pull operation");
    tracing::info!("Image name: {}", args.image_name);

    // Extract registry URL from image name
    let registry_url = extract_registry_url(&args.image_name);
    tracing::info!("Source registry: {}", registry_url);

    // Create registry client
    let client = RegistryClient::new(registry_url)?;

    // Pull the image
    client.pull_image(&args.image_name, args.output_dir.to_str().unwrap()).await?;

    tracing::info!("Successfully pulled image: {}", args.image_name);
    Ok(())
}

// Helper function to extract registry URL from image name
fn extract_registry_url(image_name: &str) -> String {
    // If image name contains a registry (like localhost:5000/myimage:tag or docker.io/myimage:tag)
    if image_name.contains('/') {
        let parts: Vec<&str> = image_name.splitn(2, '/').collect();
        let host_part = parts[0];

        // Check if it looks like a registry (contains dot or colon)
        if host_part.contains('.') || host_part.contains(':') {
            if host_part.starts_with("http://") || host_part.starts_with("https://") {
                return host_part.to_string();
            } else {
                // Assume http for localhost, https for others
                if host_part.starts_with("localhost:") || host_part.starts_with("127.0.0.1:") {
                    return format!("http://{}", host_part);
                } else {
                    return format!("https://{}", host_part);
                }
            }
        }
    }

    // Default to Docker Hub if no registry specified
    "https://registry-1.docker.io".to_string()
}
