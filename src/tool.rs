use anyhow::Result;
use bytes::Bytes;
use log::{debug, error, info};
use serde_json::Value;
use std::env::consts::{ARCH, OS};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::process::exit;
use zip::ZipArchive;

#[derive(Clone)]
pub struct Basis {
    os: &'static str,
    arch: &'static str,
    suffix: &'static str,
}

pub trait Tool {
    fn from(basis: Basis) -> Self;
    fn get_name(&self) -> String;
    fn get(&self) -> PathBuf;
    async fn init(&self) -> Result<()> {
        info!("Initializing tool: {}", self.get_name());
        if self.get().exists() {
            info!("Done");
            Ok(())
        } else {
            self.get_latest().await
        }
    }
    async fn get_latest(&self) -> Result<()>;
}

#[derive(Clone)]
pub struct BaseTool {
    basis: Basis,
    name: String,
    path: PathBuf,
}

#[derive(Clone)]
pub struct KSUD(BaseTool);

#[derive(Clone)]
pub struct MAGISKBOOT(BaseTool);

impl Tool for KSUD {
    fn from(basis: Basis) -> Self {
        let current_dir = std::env::current_dir().unwrap();

        let mut bin = PathBuf::from(current_dir)
            .join("bin")
            .join(basis.os)
            .join(basis.arch);
        bin.push(format!("{}{}", "ksud", basis.suffix));
        Self {
            0: BaseTool {
                basis: basis.clone(),
                name: "ksud".to_string(),
                path: bin,
            },
        }
    }

    fn get_name(&self) -> String {
        self.0.name.clone()
    }

    fn get(&self) -> PathBuf {
        self.0.path.clone()
    }

    async fn get_latest(&self) -> Result<()> {
        info!("Getting latest ksud");
        let api_addr = "https://api.github.com/repos/tiann/KernelSU/releases/latest".to_string();
        let assert_name = format!(
            "{}-{}-{}",
            self.0.name,
            self.0.basis.arch,
            if self.0.basis.os == "linux" {
                "unknown-linux-musl"
            } else {
                "linux-android"
            }
        );

        let assets = get_assets(api_addr).await?;
        let asset = assets
            .iter()
            .find(|asset| asset["name"].as_str() == Some(assert_name.as_str()))
            .ok_or_else(|| anyhow::anyhow!("'assets' not found in release"))?;
        info!("Downloading {}...", asset["name"].as_str().unwrap());
        let body = download_asset(asset).await?;
        if let Some(parent) = self.get().parent() {
            fs::create_dir_all(parent)?;
        }

        info!("Writing {}...", self.get().display());
        fs::write(self.get(), body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.0.path, fs::Permissions::from_mode(0o755))?;
        }
        info!("Download latest {} success", self.0.name);
        Ok(())
    }
}

impl Tool for MAGISKBOOT {
    fn from(basis: Basis) -> Self {
        let current_dir = std::env::current_dir().unwrap();

        let mut bin = PathBuf::from(current_dir)
            .join("bin")
            .join(basis.os)
            .join(basis.arch);
        bin.push(format!("{}{}", "magiskboot", basis.suffix));
        Self {
            0: BaseTool {
                basis: basis.clone(),
                name: "magiskboot".to_string(),
                path: bin,
            },
        }
    }

    fn get_name(&self) -> String {
        self.0.name.clone()
    }

    fn get(&self) -> PathBuf {
        self.0.path.clone()
    }

    async fn get_latest(&self) -> Result<()> {
        info!("Getting latest magiskboot");
        let api_addr = "https://api.github.com/repos/topjohnwu/Magisk/releases/latest".to_string();
        let assert_name = "Magisk-v";

        let assets = get_assets(api_addr).await?;
        let asset = assets
            .iter()
            .find(|asset| asset["name"].as_str().unwrap().starts_with(assert_name))
            .ok_or_else(|| anyhow::anyhow!("'assets' not found in release"))?;

        info!("Downloading {}...", asset["name"].as_str().unwrap());
        let bytes = download_asset(asset).await?;
        if let Some(parent) = self.get().parent() {
            fs::create_dir_all(parent)?;
        }

        info!("Successfully downloaded, unzipping...");
        let reader = Cursor::new(bytes);
        let mut archive = ZipArchive::new(reader)?;

        let bin_name = format!(
            "lib/{}/libmagiskboot.so",
            if self.0.basis.arch == "x86_64" {
                "x86_64"
            } else {
                "arm64-v8a"
            }
        );

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.name() == bin_name {
                let mut content = Vec::new();
                file.read_to_end(&mut content)?;
                fs::write(self.get(), content)?;
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.0.path, fs::Permissions::from_mode(0o755))?;
        }
        info!("Download latest {} success", self.0.name);
        Ok(())
    }
}

async fn get_assets(url: String) -> Result<Vec<Value>> {
    let client = reqwest::Client::builder()
        .user_agent(crate::utils::USER_AGENT)
        .build()?;
    let resp = client.get(&url).send().await?;

    if !resp.status().is_success() {
        let err_msg = format!("Failed to fetch latest release: {}", resp.status());
        error!("{}", err_msg);
        return Err(anyhow::anyhow!(err_msg));
    }
    let body = resp.text().await?;
    let release: Value = serde_json::from_str(&body)?;
    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("'assets' not found in release"))?;
    Ok(assets.clone())
}

async fn download_asset(asset: &Value) -> Result<Bytes> {
    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("'download_url' not found in release"))?;
    let client = reqwest::Client::builder()
        .user_agent(crate::utils::USER_AGENT)
        .build()?;
    let resp = client.get(download_url).send().await?;
    if !resp.status().is_success() {
        let err_msg = format!("Failed to download asset: {}", resp.status());
        error!("{}", err_msg);
        return Err(anyhow::anyhow!(err_msg));
    }
    Ok(resp.bytes().await?)
}

#[derive(Clone)]
pub struct ToolManager {
    ksud: KSUD,
    magiskboot: MAGISKBOOT,
}

impl Default for ToolManager {
    fn default() -> Self {
        let basis = Basis::default();
        let ksud = <KSUD as Tool>::from(basis.clone());
        let magiskboot = <MAGISKBOOT as Tool>::from(basis.clone());
        Self { ksud, magiskboot }
    }
}

impl ToolManager {
    pub async fn init(&self) -> Result<()> {
        debug!("Initializing tools");
        self.ksud.init().await?;
        self.magiskboot.init().await?;
        Ok(())
    }

    pub fn get_magiskboot(&self) -> MAGISKBOOT {
        self.magiskboot.clone()
    }
    pub fn get_ksud(&self) -> KSUD {
        self.ksud.clone()
    }
}

impl Default for Basis {
    fn default() -> Self {
        let os = match OS {
            "linux" => "linux",
            "android" => "android",
            _ => "Unknown",
        };
        let arch = match ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "aarch64",
            _ => "Unknown",
        };
        if os == "Unknown" || arch == "Unknown" {
            let msg = format!("Unsupported platform and arch {OS}/{ARCH}");
            println!("{msg}");
            error!("{msg}");
            exit(1);
        }
        let suffix = if os == "Windows" { ".exe" } else { "" };

        Self { os, arch, suffix }
    }
}
