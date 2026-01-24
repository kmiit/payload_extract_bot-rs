use std::path::PathBuf;
use crate::patch_boot::patch_boot;
use crate::utils::to_tg_md;
use crate::{config, payload};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::time::Duration;
use teloxide::macros::BotCommands;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Message, ResponseResult};
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
> `/patch \[url] \[partition] \[method]`
>   Patch a boot partition
>    `partition`: boot\(b\), init\_boot\(ib\), vendor\_boot\(vb\)
>    `method`: kernelsu\(k, ksu\), magisk\(m\)
>
> `/help`
>   Show this help msg\."#;

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "Dump partition(s)")]
    Dump { arg: String },
    #[command(description = "Dump partition(s)")]
    Dumper { arg: String },
    #[command(description = "patch a image")]
    Patch { arg: String },
    #[command(description = "List images in the payload")]
    List { arg: String },
    #[command(description = "Help cmd")]
    Help,
    #[command(description = "Start command")]
    Start,
    #[command(description = "quote")]
    Q,
}

pub async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    tokio::spawn(async move {
        match cmd {
            Command::Dump { arg } | Command::Dumper { arg } => {
                if let Err(e) = dump_cmd(bot, msg, arg).await {
                    error!("Error in dump_cmd: {e}");
                }
            }
            Command::Patch { arg } => {
                if let Err(e) = patch_cmd(bot, msg, arg).await {
                    error!("Error in patch_cmd: {e}");
                }
            }
            Command::List { arg } => {
                if let Err(e) = list_cmd(bot, msg, arg).await {
                    error!("Error in list_cmd: {e}");
                }
            }
            Command::Help { .. } | Command::Start { .. } => {
                if let Err(e) = help_cmd(bot, msg).await {
                    error!("Error in help_cmd: {e}");
                }
            }
            Command::Q => {
                if let Err(e) = q(bot, msg).await {
                    error!("Error in help_cmd: {e}");
                }
            }
        };
    });
    Ok(())
}

async fn q(bot: Bot, msg: Message) -> Result<Message, RequestError> {
    let msg = bot
        .send_message(msg.chat.id, "q什么q， 不准q")
        .reply_to(msg.id)
        .await?;
    tokio::time::sleep(Duration::from_secs(10)).await;
    bot.delete_message(msg.chat.id, msg.id).await?;
    return Ok(msg);
}

async fn dump_cmd(bot: Bot, msg: Message, arg: String) -> Result<Message, RequestError> {
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
    let config = config::load_config().unwrap_or_default();
    let url = cmd[0].to_string();
    let partition = cmd[1].to_string();
    let mut unsupported_partitions: Vec<String> = Vec::new();
    let partitions = partition.split(',').collect::<Vec<_>>();
    if !config.supported_partitions.is_empty() {
        let _ = partitions.iter().map(|p| {
            if config.supported_partitions.contains(&p.to_string()) {
                unsupported_partitions.push(p.to_string());
            }
        });

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

            tokio::time::sleep(Duration::from_secs(10)).await;
            bot.delete_message(msg.chat.id, status_msg.id).await?;
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

async fn list_cmd(bot: Bot, msg: Message, arg: String) -> Result<Message, RequestError> {
    let url = arg.split_whitespace().collect::<Vec<_>>()[0];
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
    let escaped_ret = ret
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let html_message = format!("<pre>{}</pre>", escaped_ret);
    bot.send_message(msg.chat.id, html_message)
        .parse_mode(ParseMode::Html)
        .reply_to(msg.id)
        .await
}

async fn patch_cmd(bot: Bot, msg: Message, arg: String) -> Result<Message, RequestError> {
    let args = arg.split_whitespace().collect::<Vec<_>>();
    let url = args[0];
    let patch_partition = args[1];
    let patch_method = if args.len() < 3 { "ksu" } else { args[2] };
    let status_msg = bot
        .send_message(
            msg.chat.id,
            format!("Patching {patch_partition} with {patch_method}"),
        )
        .reply_to(msg.id)
        .await?;
    match patch_boot(
        url.to_string(),
        patch_partition.to_string(),
        patch_method.to_string(),
    )
    .await
    {
        Ok(patched_file) => {
            info!(
                "Patch {patch_partition} with {patch_method} successfully, patched file: {}",
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
                    ">KMI: `{}`\n>Kernel Version: `{}`",
                    patched_file.kmi, patched_file.kernel_version
                )))
                .parse_mode(ParseMode::MarkdownV2);
            if PathBuf::from(patched_file.path.clone()).exists() {
                match bot.send_media_group(status_msg.chat.id, vec![InputMedia::Document(document)])
                    .reply_to(msg.id)
                    .await {
                    Ok(_) => {
                        info!("All files uploaded successfully.");
                        bot.edit_message_text(
                            status_msg.chat.id,
                            status_msg.id,
                            "All files uploaded successfully.",
                        )
                            .await?;
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        bot.delete_message(msg.chat.id, status_msg.id).await?;
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
            if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
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

async fn help_cmd(bot: Bot, msg: Message) -> Result<Message, RequestError> {
    bot.send_message(msg.chat.id, HELP_MESSAGE)
        .parse_mode(ParseMode::MarkdownV2)
        .reply_to(msg.id)
        .await
}
