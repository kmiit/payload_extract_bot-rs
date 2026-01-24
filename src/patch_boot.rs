use crate::payload::dump_partition;
use crate::tool::*;
use anyhow::Result;
use log::info;
use regex::Regex;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::process::Command;

enum PatchMethod {
    KernelSU,
    Magisk,
}

impl PatchMethod {
    fn from(s: &str) -> Result<Self> {
        match s {
            "kernelsu" | "ksu" | "k" => Ok(Self::KernelSU),
            "magisk" | "m" => Ok(Self::Magisk),
            _ => Err(anyhow::anyhow!("Invalid patch method: {}", s)),
        }
    }
    fn to_string(&self) -> String {
        match self {
            Self::KernelSU => "kernelsu".to_string(),
            Self::Magisk => "magisk".to_string(),
        }
    }
}

enum PatchPartition {
    Boot,
    InitBoot,
    VendorBoot,
}

impl PatchPartition {
    fn from(s: &str) -> Result<Self> {
        match s {
            "boot" | "b" => Ok(Self::Boot),
            "init_boot" | "ib" => Ok(Self::InitBoot),
            "vendor_boot" | "vb" => Ok(Self::VendorBoot),
            _ => Err(anyhow::anyhow!("Invalid patch partition: {}", s)),
        }
    }

    fn get_partition_name(&self) -> String {
        match self {
            Self::Boot => "boot".to_string(),
            Self::InitBoot => "init_boot".to_string(),
            Self::VendorBoot => "vendor_boot".to_string(),
        }
    }
}

struct Patch {
    method: PatchMethod,
    partition: PatchPartition,
}

pub struct PatchedFile {
    pub(crate) path: PathBuf,
    pub(crate) kmi: String,
    pub(crate) kernel_version: String,
}

impl Patch {
    fn patch(&self, dir: PathBuf) -> Result<PatchedFile> {
        let tm = ToolManager::default();
        let mut patched_name = format!(
            "{}_patched_{}",
            self.method.to_string(),
            self.partition.get_partition_name()
        );

        match &self.method {
            PatchMethod::KernelSU => {
                let ksud = tm.get_ksud().get();
                let magiskboot = tm.get_magiskboot().get();
                let (kmi, kernel_version) = get_kmi(magiskboot.clone(), dir.clone())?;

                patched_name = format!("{patched_name}-{kmi}.img");

                info!(
                    "patching {} with kmi: {}, tool: {}",
                    self.partition.get_partition_name(),
                    kmi,
                    tm.get_ksud().get().display()
                );

                let _ = Command::new(ksud)
                    .current_dir(dir.clone())
                    .args(&[
                        "boot-patch",
                        "-b",
                        format!("{}.img", self.partition.get_partition_name()).as_str(),
                        "--magiskboot",
                        magiskboot.as_path().to_str().unwrap(),
                        "--kmi",
                        kmi.as_str(),
                        "--out-name",
                        patched_name.as_str(),
                    ])
                    .output()?;
                let mut file = PathBuf::from(dir);
                file.push(&patched_name);
                Ok(PatchedFile {
                    path: file,
                    kmi,
                    kernel_version,
                })
            }
            PatchMethod::Magisk => Err(anyhow::anyhow!("Magisk patch hasn't implemented!")),
        }
    }
}

pub async fn patch_boot(
    url: String,
    patch_partition: String,
    patch_method: String,
) -> Result<PatchedFile> {
    info!("Patching boot: {url} {patch_partition} {patch_method}");
    let patch = Patch {
        method: PatchMethod::from(&patch_method)?,
        partition: PatchPartition::from(&patch_partition)?,
    };
    let mut images = Vec::new();
    images.push(patch_partition);
    images.push("boot".to_string());
    let (_, dir) = dump_partition(url.clone(), images.join(",")).await?;
    patch.patch(dir)
}

fn get_kmi(magiskboot: PathBuf, dir: PathBuf) -> Result<(String, String)> {
    info!(
        "Getting kmi from boot.img in {}, tool: {}",
        std::env::current_dir()?.display(),
        magiskboot.display()
    );
    let _ = Command::new(magiskboot)
        .current_dir(&dir)
        .args(&["unpack", "-n", "boot.img"])
        .output()?;

    let file = File::open(dir.join("kernel"))?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer)?;

    let kmi_re = Regex::new(r"(?:.* )?(\d+\.\d+)(?:\S+)?(android\d+)")?;
    let kernel_version_re = Regex::new(r"Linux version (.*)")?;

    let mut kmi: Option<String> = None;
    let mut kernel_version: Option<String> = None;

    let printable_strings = buffer
        .split(|&b| b == 0)
        .filter_map(|slice| std::str::from_utf8(slice).ok());

    for s in printable_strings {
        if kmi.is_none()
            && s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
            && let Some(caps) = kmi_re.captures(s)
            && let (Some(kernel_version_part), Some(android_version)) = (caps.get(1), caps.get(2))
        {
            let kmi_str = format!(
                "{}-{}",
                android_version.as_str(),
                kernel_version_part.as_str()
            );
            info!("Found kmi: {}", kmi_str);
            kmi = Some(kmi_str);
        }

        if kernel_version.is_none()
            && let Some(caps) = kernel_version_re.captures(s)
            && let Some(version) = caps.get(1)
        {
            let kv_str = version.as_str().trim();
            if !kv_str.is_empty() {
                info!("Found kernel version: {}", kv_str);
                kernel_version = Some(kv_str.to_string());
            }
        }

        if kmi.is_some() && kernel_version.is_some() {
            break;
        }
    }

    match (kmi, kernel_version) {
        (Some(k), Some(v)) => Ok((k, v)),
        (Some(_), None) => Err(anyhow::anyhow!("Can't parse kernel version from kernel")),
        (None, Some(_)) => Err(anyhow::anyhow!("Can't parse kmi from boot.img")),
        (None, None) => Err(anyhow::anyhow!(
            "Can't parse kmi and kernel version from boot.img"
        )),
    }
}
