use anyhow::Result;
use oci_spec::image::{ImageConfiguration, ImageManifest};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: String,
    pub digest: String,
    pub size: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub id: String,
    pub name: String,
    pub layers: Vec<Layer>,
    pub config: ImageConfiguration,
    pub manifest: ImageManifest,
}

#[derive(Debug)]
pub struct StorageManager {
    root_dir: PathBuf,
    layers_dir: PathBuf,
    images_dir: PathBuf,
}

impl StorageManager {
    pub fn new(root_dir: PathBuf) -> Result<Self> {
        let layers_dir = root_dir.join("layers");
        let images_dir = root_dir.join("images");

        Ok(Self {
            root_dir,
            layers_dir,
            images_dir,
        })
    }

    pub async fn init(&self) -> Result<()> {
        // Create necessary directories
        fs::create_dir_all(&self.layers_dir).await?;
        fs::create_dir_all(&self.images_dir).await?;
        Ok(())
    }

    pub async fn create_layer(&self, data: &[u8]) -> Result<Layer> {
        use sha2::{Digest, Sha256};
        
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash_result = hasher.finalize();
        let digest = format!("sha256:{:x}", hash_result);
        
        let layer_id = uuid::Uuid::new_v4().to_string();
        let layer_path = self.layers_dir.join(format!("{}.tar.gz", layer_id));
        
        // Compress and save the layer data
        use std::io::Write;
        let mut gz_encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz_encoder.write_all(data)?;
        let compressed_data = gz_encoder.finish()?;
        
        fs::write(&layer_path, compressed_data).await?;
        
        Ok(Layer {
            id: layer_id,
            digest,
            size: data.len() as u64,
            path: layer_path,
        })
    }

    pub async fn save_image(&self, image: &Image) -> Result<()> {
        let image_path = self.images_dir.join(&image.id);
        fs::create_dir_all(&image_path).await?;

        // Save image config
        let config_path = image_path.join("config.json");
        let config_json = serde_json::to_string_pretty(&image.config)?;
        fs::write(&config_path, config_json).await?;

        // Save image manifest
        let manifest_path = image_path.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(&image.manifest)?;
        fs::write(&manifest_path, manifest_json).await?;

        // Save image name for lookup
        let name_path = image_path.join("name.txt");
        fs::write(&name_path, &image.name).await?;

        Ok(())
    }

    pub async fn get_image(&self, id: &str) -> Result<Option<Image>> {
        let image_path = self.images_dir.join(id);
        if !image_path.exists() {
            return Ok(None);
        }

        // Read image config
        let config_path = image_path.join("config.json");
        let config_content = fs::read_to_string(&config_path).await?;
        let config: ImageConfiguration = serde_json::from_str(&config_content)?;

        // Read image manifest
        let manifest_path = image_path.join("manifest.json");
        let manifest_content = fs::read_to_string(&manifest_path).await?;
        let manifest: ImageManifest = serde_json::from_str(&manifest_content)?;

        // For now, return a minimal image - in a real implementation we'd reconstruct the layers
        Ok(Some(Image {
            id: id.to_string(),
            name: id.to_string(), // Placeholder
            layers: vec![], // Placeholder
            config,
            manifest,
        }))
    }

    pub async fn get_image_by_name(&self, name: &str) -> Result<Option<Image>> {
        // List all images in the storage
        let image_ids = self.list_images().await?;

        // Look for an image with the matching name
        for id in image_ids {
            let image_path = self.images_dir.join(&id);
            let name_path = image_path.join("name.txt"); // Assuming we store the name

            if name_path.exists() {
                let stored_name = fs::read_to_string(&name_path).await?;
                if stored_name.trim() == name {
                    return self.get_image(&id).await;
                }
            }
        }

        // If we don't have name mapping, try to find by ID (last part of name)
        if let Some(last_slash) = name.rfind('/') {
            let image_id = &name[last_slash + 1..];
            if let Some(dot_pos) = image_id.find(':') {
                let id_part = &image_id[..dot_pos];
                if let Ok(_) = self.get_image(id_part).await {
                    // This is a simplified approach - in practice we'd need better name-to-id mapping
                    // For now, return the first image we find
                    if let Some(first_id) = self.list_images().await?.first() {
                        return self.get_image(first_id).await;
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn clone_for_build(&self) -> StorageManager {
        StorageManager {
            root_dir: self.root_dir.clone(),
            layers_dir: self.layers_dir.clone(),
            images_dir: self.images_dir.clone(),
        }
    }

    pub async fn list_images(&self) -> Result<Vec<String>> {
        let mut images = Vec::new();
        let mut entries = fs::read_dir(&self.images_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                images.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        
        Ok(images)
    }

    pub async fn remove_image(&self, id: &str) -> Result<()> {
        let image_path = self.images_dir.join(id);
        if image_path.exists() {
            fs::remove_dir_all(&image_path).await?;
        }
        Ok(())
    }

    pub async fn gc(&self) -> Result<u64> {
        // Placeholder for garbage collection
        // In a real implementation, this would remove unused layers and images
        Ok(0) // Return number of bytes freed
    }
}