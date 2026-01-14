use anyhow::Result;
use oci_spec::image::{ImageManifest, ImageConfiguration, Descriptor, MediaType};
use reqwest;
use serde_json;
use sha2::{Digest, Sha256};

pub struct RegistryClient {
    client: reqwest::Client,
    registry_url: String,
}

impl RegistryClient {
    pub fn new(registry_url: String) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            registry_url: registry_url.trim_end_matches('/').to_string(),
        })
    }

    pub async fn push_image(&self, image_name: &str, image: &crate::storage::Image) -> Result<()> {
        println!("Pushing image {} to registry...", image_name);

        // Parse the image name to extract repository and tag
        let (repo, tag) = self.parse_image_name(image_name)?;

        // Upload each layer
        for layer in &image.layers {
            self.upload_layer(&repo, layer).await?;
        }

        // Upload image config
        let config_digest = self.upload_config(&repo, &image.config).await?;

        // Create and upload manifest
        let manifest = self.create_manifest(&image.config, &image.layers, &config_digest)?;
        self.upload_manifest(&repo, &tag, &manifest).await?;

        println!("Successfully pushed image {} to registry", image_name);
        Ok(())
    }

    fn parse_image_name(&self, image_name: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = image_name.rsplitn(2, ':').collect();
        let (tag, repo) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("latest", image_name)
        };

        // Handle registry prefixes (e.g., localhost:5000/myimage:tag)
        let repo_parts: Vec<&str> = repo.splitn(2, '/').collect();
        if repo_parts.len() == 1 || !repo_parts[0].contains('.') && !repo_parts[0].contains(':') {
            // No registry prefix, assume library namespace
            Ok(("library/".to_string() + repo, tag.to_string()))
        } else {
            // Has registry prefix, extract just the repository name
            // e.g., localhost:5000/myimage -> myimage
            Ok((repo_parts[1].to_string(), tag.to_string()))
        }
    }

    async fn upload_layer(&self, repo: &str, layer: &crate::storage::Layer) -> Result<()> {
        println!("Uploading layer {}...", layer.digest);

        // Step 1: Initiate upload
        let upload_url = format!("{}/v2/{}/blobs/uploads/", self.registry_url, repo);
        let response = self.client.post(&upload_url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to initiate upload: {} - {}", status, error_text));
        }

        let location_header = response.headers().get("location")
            .ok_or_else(|| anyhow::anyhow!("Missing location header in upload initiation response"))?;
        let location = location_header.to_str()
            .map_err(|e| anyhow::anyhow!("Invalid location header: {}", e))?;

        // Construct absolute URL if location is relative
        let absolute_location = if location.starts_with("http") {
            location.to_string()
        } else {
            format!("{}{}", self.registry_url, location)
        };

        // Step 2: Upload the layer data
        let layer_data = tokio::fs::read(&layer.path).await?;
        let response = self.client
            .put(&absolute_location)
            .header("content-type", "application/octet-stream")
            .query(&[("digest", &layer.digest)])
            .body(layer_data)
            .send()
            .await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to upload layer: {} - {}", status, error_text));
        }

        println!("Successfully uploaded layer {}", layer.digest);
        Ok(())
    }

    async fn upload_config(&self, repo: &str, config: &ImageConfiguration) -> Result<String> {
        println!("Uploading image config for repo {}...", repo);

        let config_json = serde_json::to_vec(config)?;

        // Calculate digest of config
        let mut hasher = Sha256::new();
        hasher.update(&config_json);
        let hash = hasher.finalize();
        let config_digest = format!("sha256:{:x}", hash);

        // Upload config as blob to the specific repository
        let upload_url = format!("{}/v2/{}/blobs/uploads", self.registry_url, repo); // Removed trailing slash
        let response = self.client.post(&upload_url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to initiate config upload: {} - {}", status, error_text));
        }

        let location_header = response.headers().get("location")
            .ok_or_else(|| anyhow::anyhow!("Missing location header in config upload initiation"))?;
        let location = location_header.to_str()
            .map_err(|e| anyhow::anyhow!("Invalid location header: {}", e))?;

        // Construct absolute URL if location is relative
        let absolute_location = if location.starts_with("http") {
            location.to_string()
        } else {
            format!("{}{}", self.registry_url, location)
        };

        let response = self.client
            .put(&absolute_location)
            .header("content-type", "application/octet-stream")
            .query(&[("digest", &config_digest)])
            .body(config_json)
            .send()
            .await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to upload config: {} - {}", status, error_text));
        }

        println!("Successfully uploaded config with digest {}", config_digest);
        Ok(config_digest)
    }

    fn create_manifest(&self, config: &ImageConfiguration, layers: &[crate::storage::Layer], config_digest: &str) -> Result<ImageManifest> {
        use oci_spec::image::{ImageManifestBuilder, DescriptorBuilder, Digest};

        let layer_descriptors: Vec<Descriptor> = layers.iter().map(|layer| {
            DescriptorBuilder::default()
                .media_type(MediaType::ImageLayerGzip)
                .size(layer.size)  // Use u64 directly
                .digest(Digest::try_from(layer.digest.clone()).unwrap())  // Convert string to Digest
                .build()
                .unwrap() // In a real implementation, handle this error properly
        }).collect();

        // Calculate config size
        let config_json = serde_json::to_vec(config)?;
        let config_size = config_json.len() as u64; // Use u64 directly

        let config_descriptor = DescriptorBuilder::default()
            .media_type(MediaType::ImageConfig)
            .size(config_size)
            .digest(Digest::try_from(config_digest.to_string()).unwrap())  // Convert string to Digest
            .build()
            .unwrap(); // In a real implementation, handle this error properly

        let manifest = ImageManifestBuilder::default()
            .schema_version(2u32)
            .media_type(MediaType::ImageManifest)
            .config(config_descriptor)
            .layers(layer_descriptors)
            .build()?;

        Ok(manifest)
    }

    async fn upload_manifest(&self, repo: &str, tag: &str, manifest: &ImageManifest) -> Result<()> {
        println!("Uploading manifest for {}:{}...", repo, tag);

        let manifest_json = serde_json::to_vec(manifest)?;

        let url = format!("{}/v2/{}/manifests/{}", self.registry_url, repo, tag);
        let response = self.client
            .put(&url)
            .header("content-type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest_json)
            .send()
            .await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to upload manifest: {} - {}", status, error_text));
        }

        println!("Successfully uploaded manifest for {}:{}", repo, tag);
        Ok(())
    }

    pub async fn pull_image(&self, image_name: &str, output_dir: &str) -> Result<()> {
        println!("Pulling image {} from registry...", image_name);

        // Parse the image name to extract repository and tag
        let (repo, tag) = self.parse_image_name(image_name)?;

        // Download the manifest
        let manifest = self.download_manifest(&repo, &tag).await?;

        // Download each layer
        for layer_descriptor in manifest.layers() {
            self.download_layer(&repo, layer_descriptor, output_dir).await?;
        }

        // Download config
        self.download_config(&repo, manifest.config(), output_dir).await?;

        println!("Successfully pulled image {} from registry", image_name);
        Ok(())
    }

    async fn download_manifest(&self, repo: &str, tag: &str) -> Result<oci_spec::image::ImageManifest> {
        println!("Downloading manifest for {}:{}...", repo, tag);

        let url = format!("{}/v2/{}/manifests/{}", self.registry_url, repo, tag);
        let response = self.client.get(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to download manifest: {} - {}", status, error_text));
        }

        let manifest_bytes = response.bytes().await?;
        let manifest: oci_spec::image::ImageManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse manifest: {}", e))?;

        println!("Successfully downloaded manifest for {}:{}", repo, tag);
        Ok(manifest)
    }

    async fn download_layer(&self, repo: &str, layer_descriptor: &oci_spec::image::Descriptor, output_dir: &str) -> Result<()> {
        println!("Downloading layer {}...", layer_descriptor.digest());

        let url = format!("{}/v2/{}/blobs/{}", self.registry_url, repo, layer_descriptor.digest());
        let response = self.client.get(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to download layer: {} - {}", status, error_text));
        }

        let layer_data = response.bytes().await?;

        // Create output directory if it doesn't exist
        tokio::fs::create_dir_all(output_dir).await?;

        // Save layer to file - convert digest to string for filename
        let digest_str = layer_descriptor.digest().as_ref();
        let layer_filename = format!("{}/layer_{}.tar.gz", output_dir, digest_str.replace(":", "_"));
        tokio::fs::write(&layer_filename, layer_data).await?;

        println!("Successfully downloaded layer {} to {}", layer_descriptor.digest(), layer_filename);
        Ok(())
    }

    async fn download_config(&self, repo: &str, config_descriptor: &oci_spec::image::Descriptor, output_dir: &str) -> Result<()> {
        println!("Downloading config {}...", config_descriptor.digest());

        let url = format!("{}/v2/{}/blobs/{}", self.registry_url, repo, config_descriptor.digest());
        let response = self.client.get(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to download config: {} - {}", status, error_text));
        }

        let config_data = response.bytes().await?;

        // Save config to file - convert digest to string for filename
        let digest_str = config_descriptor.digest().as_ref();
        let config_filename = format!("{}/config_{}.json", output_dir, digest_str.replace(":", "_"));
        tokio::fs::write(&config_filename, config_data).await?;

        println!("Successfully downloaded config {} to {}", config_descriptor.digest(), config_filename);
        Ok(())
    }
}