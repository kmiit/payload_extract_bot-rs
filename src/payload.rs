use crate::utils;
use anyhow::Result;
use log::{debug, info};
use payload_dumper::extractor::remote::{extract_partition_remote_zip, list_partitions_remote_zip};
use serde_json::Value;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, thread};
use tokio::sync::oneshot;

pub struct PartitionInfo {
    pub name: String,
    pub size: u64,
    pub hash: Option<String>,
    pub path: PathBuf,
}

pub async fn dump_partition(
    url: String,
    partition: String,
) -> Result<(Vec<PartitionInfo>, PathBuf)> {
    let mut partitions: Vec<String> = partition.split(',').map(|s| s.to_string()).collect();
    partitions.sort();
    partitions.dedup();
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let temp_dir = PathBuf::from("tmp").join(ts.to_string());
    fs::create_dir_all(&temp_dir)?;
    info!("Dumping partitions to {}", temp_dir.display());

    let rom_info = get_rom_info(url.clone()).await?;
    let all_partitions_info = rom_info["partitions"].as_array().unwrap();

    let mut files = Vec::new();
    let mut receivers = Vec::new();

    for p_name in partitions {
        let out_put = temp_dir.join(format!("{p_name}.img"));
        if let Some(part_info) = all_partitions_info.iter().find(|p| p["name"] == p_name) {
            let info = PartitionInfo {
                name: p_name.clone(),
                size: part_info["size_bytes"].as_u64().unwrap_or(0),
                hash: part_info["hash"].as_str().map(|s| s.to_string()),
                path: out_put.clone(),
            };
            files.push(info);

            let url_clone = url.clone();
            let (tx, rx) = oneshot::channel();
            thread::spawn(move || {
                let result = extract_partition_remote_zip(
                    url_clone,
                    &p_name,
                    out_put,
                    Option::from(utils::USER_AGENT),
                    None,
                    None,
                    None::<PathBuf>,
                );
                let _ = tx.send(result);
            });
            receivers.push(rx);
        }
    }

    for rx in receivers {
        rx.await??;
    }

    Ok((files, temp_dir))
}

pub async fn list_image(url: String) -> Result<String> {
    info!("Listing image: {url}");
    let info = get_rom_info(url).await?;
    let partitions = info["partitions"].as_array().unwrap();
    let partitions_str = partitions
        .iter()
        .map(|p| {
            format!(
                "  - {}: {}",
                p["name"].as_str().unwrap(),
                p["size_readable"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let total = info["total_partitions"].as_u64().unwrap();
    let size = info["total_size_readable"].as_str().unwrap();
    let security_patch = info["security_patch_level"].as_str().unwrap();
    let ret = format!(
        "Total size: {size}\nSecurity patch level: {security_patch}\nTotal partitions: {total}\nPartitions:\n{partitions_str}"
    );
    debug!("{ret}");
    Ok(ret)
}

async fn get_rom_info(url: String) -> Result<Value> {
    info!("Getting rom info: {url}");

    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        let result = list_partitions_remote_zip(url, Option::from(utils::USER_AGENT), None);
        let _ = tx.send(result);
    });

    let partition_list = rx.await??;

    Ok(serde_json::from_str(partition_list.json.as_str())?)
}
