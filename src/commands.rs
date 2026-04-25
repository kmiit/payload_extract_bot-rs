use crate::cmd_impl::*;
use crate::utils::*;
use crate::{config, tool};
use log::error;
use std::sync::Arc;
use teloxide::Bot;
use teloxide::macros::BotCommands;
use teloxide::prelude::{Message, ResponseResult};

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "quote")]
    Q,
    #[command(description = "Dump partition(s)")]
    Dump { arg: String },
    #[command(description = "Dump partition(s)")]
    Dumper { arg: String },
    #[command(description = "patch a image")]
    Patch { arg: String },
    #[command(description = "List images in the payload")]
    List { arg: String },
    #[command(description = "Show OTA metadata (short)")]
    Meta { arg: String },
    #[command(description = "Show OTA metadata")]
    MetaData { arg: String },
    #[command(description = "Help cmd")]
    Help,
    #[command(description = "Start command")]
    Start,
    #[command(description = "Update ksud and magiskboot tools")]
    Update,
    #[command(description = "Show current bot status")]
    Status,
}

pub async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    tm: Arc<tool::ToolManager>,
    cfg: Arc<config::Config>,
) -> ResponseResult<()> {
    log_message(msg.clone()).await?;
    tokio::spawn(async move {
        match cmd {
            Command::Q => {
                if let Err(e) = q(bot, msg).await {
                    error!("Error in help_cmd: {e}");
                }
            }
            Command::Dump { arg } | Command::Dumper { arg } => {
                if let Err(e) = dump_cmd(bot, msg, arg, cfg).await {
                    error!("Error in dump_cmd: {e}");
                }
            }
            Command::Patch { arg } => {
                if let Err(e) = patch_cmd(bot, msg, arg, tm).await {
                    error!("Error in patch_cmd: {e}");
                }
            }
            Command::List { arg } => {
                if let Err(e) = list_cmd(bot, msg, arg).await {
                    error!("Error in list_cmd: {e}");
                }
            }
            Command::MetaData { arg } => {
                if let Err(e) = meta_cmd(bot, msg, arg).await {
                    error!("Error in meta_data_cmd: {e}");
                }
            }
            Command::Meta { arg } => {
                if let Err(e) = meta_cmd(bot, msg, arg).await {
                    error!("Error in meta_cmd: {e}");
                }
            }
            Command::Help | Command::Start => {
                if let Err(e) = help_cmd(bot, msg).await {
                    error!("Error in help_cmd: {e}");
                }
            }
            Command::Update => {
                if let Err(e) = update_cmd(bot, msg, tm, cfg).await {
                    error!("Error in update_cmd: {e}");
                }
            }
            Command::Status => {
                if let Err(e) = status_cmd(bot, msg, cfg).await {
                    error!("Error in status_cmd: {e}");
                }
            }
        };
    });
    Ok(())
}
