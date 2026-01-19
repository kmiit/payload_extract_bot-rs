# Payload Extract Bot

A Telegram bot that can extract partitions from a `payload.bin` file from a given URL.

## Features

- List partitions from a URL.
- Dump one or more partitions.
- Patch boot partitions with KernelSU or Magisk.

## Usage

The bot understands the following commands:

| Command                             | Description                                                               | Example                        |
|:------------------------------------|:--------------------------------------------------------------------------|:-------------------------------|
| `/dump [url] [partitions]`          | Dump partition(s) from the URL. Partitions can be a comma-separated list. | `/dump <url> boot,vendor_boot` |
| `/list [url]`                       | List all available partitions from the URL.                               | `/list <url>`                  |
| `/patch [url] [partition] <method>` | Patch a boot partition.                                                   | `/patch <url> boot ksu`        |
| `/help`                             | Show the help message.                                                    | `/help`                        |

### Patch Command Details

- **`partition`**: `boot` (or `b`), `init_boot` (or `ib`), `vendor_boot` (or `vb`)
- **`method`**: `kernelsu` (or `k`, `ksu`), `magisk` (or `m`), default is `kernelsu`

## Configuration

You need to create a `config.toml` file in the root directory. You can copy `config.toml.example` to get started.

```toml
# Telegram bot token
TOKEN = "YOUR_TELEGRAM_BOT_TOKEN"

# (Optional) Telegram Bot API URL.
# Required if you want to upload files larger than 50MB.
API_URL = "https://api.telegram.org"

# (Optional) Log level. Default is "debug".
# Can be "trace", "debug", "info", "warn", "error".
RUST_LOG = "debug"

# (Optional) Whitelist of supported partitions.
# Leave blank to support all partitions.
# Example: ["boot", "vendor_boot", "system"]
SUPPORTED_PARTITIONS = []
```

## Build

This project is built with Rust.

> Currently, only Linux on x86_64 and aarch64 is officially supported for building.

```shell
cargo build --release
```

After building, you can find the executable at `target/release/payload_extract_bot`.

## Running the Bot

1. Make sure you have created and configured your `config.toml`.
2. Run the bot:

```shell
./target/release/payload_extract_bot
```

## Thanks

- [teloxide](https://github.com/teloxide/teloxide)
- [payload_dumper](https://github.com/rhythmcache/payload-dumper-rust)
- [kernelsu](https://github.com/tiann/KernelSU)
- [magisk](https://github.com/topjohnwu/Magisk)

## Contributing

Contributions are welcome! Please feel free to open an issue or submit a pull request.
