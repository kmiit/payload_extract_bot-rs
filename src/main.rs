mod commands;
mod config;
mod patch_boot;
mod payload;
mod tool;
mod utils;

use crate::commands::{Command, answer};
use anyhow::Result;
use log::info;
use std::time::Duration;
use teloxide::dispatching::{Dispatcher, HandlerExt, UpdateFilterExt};
use teloxide::prelude::{Bot, Update};
use teloxide::{dptree, net};

#[tokio::main]
async fn main() -> Result<()> {
    let config = config::load_config()?;
    pretty_env_logger::init();
    info!("Initializing tools");
    let tm = tool::ToolManager::default();
    tm.init().await?;
    info!("Cleaning temp files");
    std::fs::remove_dir_all("tmp").ok();
    info!("Starting command bot...");
    let client = net::default_reqwest_settings().timeout(Duration::from_secs(120));
    let bot = Bot::with_client(config.token, client.build()?).set_api_url(config.api_url.parse()?);

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(answer));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}
