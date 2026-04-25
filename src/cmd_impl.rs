use crate::patch_boot::patch_boot;
use crate::utils::*;
use crate::{config, payload, tool};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::Message;
use teloxide::requests::Requester;
use teloxide::sugar::request::RequestReplyExt;
use teloxide::types::{InputFile, InputMedia, InputMediaDocument, ParseMode};
use teloxide::{Bot, RequestError};

const HELP_MESSAGE: &str = r#"*[Payload dumper bot written in rust](https://github.com/kmiit/payload_dump_bot-rs)\.*

> **Usage:**
> `/dump \[url] \[partition1<,partition2,partition3\.\.\.>]`
>   Dump partition\(s\) from url
>
> `/list \[url]`
>   List partition info of url
>
> `/meta \[url]`
>   Show OTA metadata from the OTA zip
>
> `/patch \[url] \[partition] \[kmi]`
>   Patch a boot partition with KernelSU
>    `partition`: boot\(b\), init\_boot\(ib\), vendor\_boot\(vb\)
>    `kmi`: optional
>
> `/update`
>   Update ksud and magiskboot tools to latest version
>
> `/help`
>   Show this help msg\."#;

pub async fn status_cmd(
    bot: Bot,
    msg: Message,
    cfg: Arc<config::Config>,
) -> Result<Message, RequestError> {
    if !is_admin(msg.clone(), cfg) {
        return Ok(msg);
    }
    let sys_info = get_sysinfo();
    bot.send_message(msg.chat.id, format!("{}", sys_info))
        .await?;
    Ok(msg)
}

pub async fn dump_cmd(
    bot: Bot,
    msg: Message,
    arg: String,
    cfg: Arc<config::Config>,
) -> Result<Message, RequestError> {
    let cmd: Vec<&str> = arg.split_whitespace().collect();
    if cmd.len() != 2 {
        warn!("{}: Dump: Invalid command: {arg}", msg.chat.id);
        let msg = bot
            .send_message(
                msg.chat.id,
                "Invalid command! Usage: /dump <url> <partition1,partition2,...>",
            )
            .reply_to(msg.id)
            .await?;
        tokio::time::sleep(Duration::from_secs(10)).await;
        bot.delete_message(msg.chat.id, msg.id).await?;
        return Ok(msg);
    }
    let url = cmd[0].to_string();
    let partition = cmd[1].to_string();
    let mut unsupported_partitions: Vec<String> = Vec::new();
    let partitions = partition.split(',').collect::<Vec<_>>();
    if !cfg.supported_partitions.is_empty() {
        unsupported_partitions.extend(
            partitions
                .iter()
                .filter(|p| !cfg.supported_partitions.iter().any(|item| item == **p))
                .map(|p| (*p).to_string()),
        );

        if !unsupported_partitions.is_empty() {
            warn!(
                "{}: Dump: Partition {} is not supported",
                msg.chat.id,
                unsupported_partitions.join(", ")
            );
            let msg = bot
                .send_message(
                    msg.chat.id,
                    format!("Partition {partition} is not supported!"),
                )
                .reply_to(msg.id)
                .await?;
            tokio::time::sleep(Duration::from_secs(10)).await;
            bot.delete_message(msg.chat.id, msg.id).await?;
            return Ok(msg);
        }
    }
    info!(
        "{}: Received dump command, url: {url}, partition: {partition}",
        msg.chat.id
    );
    debug!(
        "{}: Sender: {}, chat_id: {}",
        msg.id,
        msg.from.unwrap().id,
        msg.chat.id
    );
    let status_msg = bot
        .send_message(msg.chat.id, format!("Dumping {partition}..."))
        .reply_to(msg.id)
        .await?;
    match payload::dump_partition(url, partition).await {
        Ok((files, temp_dir)) => {
            let num_files = files.len();
            info!(
                "Successfully dumped {num_files} files to {}",
                temp_dir.display()
            );

            if num_files == 0 {
                bot.send_message(msg.chat.id, "No dumped file found.")
                    .await?;
            } else {
                bot.edit_message_text(
                    status_msg.chat.id,
                    status_msg.id,
                    format!("Partitions dumped successfully! Uploading {num_files} files...",),
                )
                .await?;
                let mut caption = String::new();
                for path in files.iter().clone() {
                    caption.push_str(&format!(
                        "> `{}`\\(`{}`\\): `{}`\n>\n",
                        path.name,
                        path.size,
                        path.hash.as_deref().unwrap_or("N/A").trim_matches('"')
                    ));
                }

                let mut media: Vec<InputMedia> = Vec::with_capacity(files.len());

                for (idx, path) in files.iter().enumerate() {
                    let document = InputMediaDocument::new(InputFile::file(path.path.clone()));
                    if idx == files.len() - 1 {
                        media.push(InputMedia::Document(
                            document
                                .caption(caption.clone())
                                .parse_mode(ParseMode::MarkdownV2),
                        ));
                    } else {
                        media.push(InputMedia::Document(document));
                    }
                }

                match bot
                    .send_media_group(msg.chat.id, media)
                    .reply_to(msg.id)
                    .await
                {
                    Ok(_) => {
                        info!("All files uploaded successfully.");
                        bot.edit_message_text(
                            status_msg.chat.id,
                            status_msg.id,
                            "All files uploaded successfully.",
                        )
                        .await?;
                    }
                    Err(err) => {
                        error!("Error while uploading files: {err}");
                        bot.edit_message_text(
                            status_msg.chat.id,
                            status_msg.id,
                            format!("Failed to upload file: {err}"),
                        )
                        .await?;
                    }
                }
            };

            delete_later(&bot, msg.chat.id, status_msg.id).await?;
            info!("Cleaning up temporary directory: {}", temp_dir.display());
            if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
                error!(
                    "Failed to clean up temp directory {}: {e}",
                    temp_dir.display(),
                );
            }
        }
        Err(e) => {
            error!("Failed to dump partitions: {e}");
            bot.edit_message_text(
                status_msg.chat.id,
                status_msg.id,
                format!("Failed to dump partitions: {e}"),
            )
            .await?;
        }
    }
    Ok(status_msg)
}

pub async fn meta_cmd(bot: Bot, msg: Message, arg: String) -> Result<Message, RequestError> {
    let Some(url) = arg.split_whitespace().next() else {
        return bot
            .send_message(msg.chat.id, "Invalid command! Usage: /meta <url>")
            .reply_to(msg.id)
            .await;
    };
    info!("{}: Received meta command, url: {url}", msg.chat.id);
    debug!(
        "{}: Sender: {}, chat_id: {}",
        msg.id,
        msg.from.unwrap().id,
        msg.chat.id
    );
    let status_msg = bot
        .send_message(msg.chat.id, "Fetching OTA metadata...")
        .reply_to(msg.id)
        .await?;
    match payload::read_ota_metadata(url.to_string()).await {
        Ok(text) => {
            let escaped = text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let html_message = format!("<blockquote expandable>{escaped}</blockquote>");
            bot.send_message(msg.chat.id, html_message)
                .parse_mode(ParseMode::Html)
                .reply_to(msg.id)
                .await?;
            delete_later(&bot, status_msg.chat.id, status_msg.id).await?;
        }
        Err(e) => {
            error!("Failed to fetch OTA metadata: {e}");
            bot.edit_message_text(
                status_msg.chat.id,
                status_msg.id,
                format!("Failed to fetch OTA metadata: {e}"),
            )
            .await?;
        }
    }
    Ok(status_msg)
}

pub async fn list_cmd(bot: Bot, msg: Message, arg: String) -> Result<Message, RequestError> {
    let Some(url) = arg.split_whitespace().next() else {
        return bot
            .send_message(msg.chat.id, "Invalid command! Usage: /list <url>")
            .reply_to(msg.id)
            .await;
    };
    info!("{}: Received list command, url: {url}", msg.chat.id);
    debug!(
        "{}: Sender: {}, chat_id: {}",
        msg.id,
        msg.from.unwrap().id,
        msg.chat.id
    );
    let ret = payload::list_image(url.to_string())
        .await
        .unwrap_or_else(|e| format!("Error fetching image: {e}"));
    let escaped = ret
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let html_message = format!("<blockquote expandable>{escaped}</blockquote>");
    bot.send_message(msg.chat.id, html_message)
        .parse_mode(ParseMode::Html)
        .reply_to(msg.id)
        .await
}

pub async fn patch_cmd(
    bot: Bot,
    msg: Message,
    arg: String,
    tm: Arc<tool::ToolManager>,
) -> Result<Message, RequestError> {
    let args = arg.split_whitespace().collect::<Vec<_>>();
    if args.len() < 2 || args.len() > 3 {
        return bot
            .send_message(
                msg.chat.id,
                "Invalid command! Usage: /patch <url> <partition> [kmi] \nOnly support ksu patch now.",
            )
            .reply_to(msg.id)
            .await;
    }

    let url = args[0];
    let patch_partition = args[1];
    let patch_kmi = args.get(2).map(|s| (*s).to_string());
    let status_msg = bot
        .send_message(
            msg.chat.id,
            match &patch_kmi {
                Some(kmi) => format!("Patching {patch_partition} with KernelSU (KMI: {kmi})"),
                None => format!("Patching {patch_partition} with KernelSU"),
            },
        )
        .reply_to(msg.id)
        .await?;
    match patch_boot(url.to_string(), patch_partition.to_string(), patch_kmi, tm).await {
        Ok(patched_file) => {
            info!(
                "Patch {patch_partition} with KernelSU successfully, patched file: {}",
                patched_file.path.display()
            );
            bot.edit_message_text(
                status_msg.chat.id,
                status_msg.id,
                format!("Patch {patch_partition} successfully, uploading..."),
            )
            .await?;
            let document = InputMediaDocument::new(InputFile::file(patched_file.path.clone()))
                .caption(to_tg_md(format!(
                    ">Patch Method: `{}`\n>Patch Version: `{}`\n>KMI: `{}`\n>Kernel Version: `{}`",
                    patched_file.patch_method,
                    patched_file.patch_version,
                    patched_file.kmi,
                    patched_file.kernel_version
                )))
                .parse_mode(ParseMode::MarkdownV2);
            if patched_file.path.exists() {
                match bot
                    .send_media_group(status_msg.chat.id, vec![InputMedia::Document(document)])
                    .reply_to(msg.id)
                    .await
                {
                    Ok(_) => {
                        info!("All files uploaded successfully.");
                        bot.edit_message_text(
                            status_msg.chat.id,
                            status_msg.id,
                            "All files uploaded successfully.",
                        )
                        .await?;
                        delete_later(&bot, msg.chat.id, status_msg.id).await?;
                    }
                    Err(err) => {
                        error!("Error while uploading files: {err}");
                        bot.edit_message_text(
                            status_msg.chat.id,
                            status_msg.id,
                            format!("Failed to upload file: {err}"),
                        )
                        .await?;
                    }
                }
            } else {
                bot.edit_message_text(
                    status_msg.chat.id,
                    status_msg.id,
                    format!("Patched file {} not found!", patched_file.path.display()),
                )
                .await?;
            }

            let temp_dir = patched_file.path.parent().unwrap();
            info!("Cleaning up temporary directory: {}", temp_dir.display());
            if let Err(e) = std::fs::remove_dir_all(temp_dir) {
                error!(
                    "Failed to clean up temp directory {}: {e}",
                    temp_dir.display(),
                );
            }
        }
        Err(e) => {
            error!("Failed to patch {patch_partition}: {e}");
            bot.edit_message_text(
                status_msg.chat.id,
                status_msg.id,
                format!("Failed to patch {patch_partition}: {e}"),
            )
            .await?;
        }
    };
    Ok(status_msg)
}

pub async fn help_cmd(bot: Bot, msg: Message) -> Result<Message, RequestError> {
    bot.send_message(msg.chat.id, HELP_MESSAGE)
        .parse_mode(ParseMode::MarkdownV2)
        .reply_to(msg.id)
        .await
}

pub async fn update_cmd(
    bot: Bot,
    msg: Message,
    tm: Arc<tool::ToolManager>,
    cfg: Arc<config::Config>,
) -> Result<Message, RequestError> {
    if !is_admin(msg.clone(), cfg) {
        return Ok(msg);
    }
    let status_msg = bot
        .send_message(msg.chat.id, "Updating tools...")
        .reply_to(msg.id)
        .await?;
    match tm.update().await {
        Ok(()) => {
            bot.edit_message_text(
                status_msg.chat.id,
                status_msg.id,
                "Tools updated successfully!",
            )
            .await?;
        }
        Err(e) => {
            error!("Failed to update tools: {e}");
            bot.edit_message_text(
                status_msg.chat.id,
                status_msg.id,
                format!("Failed to update tools: {e}"),
            )
            .await?;
        }
    }
    Ok(status_msg)
}

async fn delete_later(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    message_id: teloxide::types::MessageId,
) -> Result<(), RequestError> {
    tokio::time::sleep(Duration::from_secs(10)).await;
    bot.delete_message(chat_id, message_id).await?;
    Ok(())
}

pub async fn q(bot: Bot, msg: Message) -> Result<Message, RequestError> {
    let msg = bot
        .send_message(msg.chat.id, "q什么q， 不准q")
        .reply_to(msg.id)
        .await?;
    tokio::time::sleep(Duration::from_secs(10)).await;
    bot.delete_message(msg.chat.id, msg.id).await?;
    return Ok(msg);
}
