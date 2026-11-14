# zw-vhost

A lightweight, fast CLI tool written in Rust to automate creating, listing, and deleting local Nginx virtual hosts (`.test` domains) on Ubuntu and WSL.

## Features
- **Instant Virtual Hosts:** Automatically generates `/etc/nginx/sites-available/<domain>` and symlinks to `sites-enabled`.
- **Safe Configuration:** Validates Nginx syntax (`nginx -t`) before reloading, with automatic rollback if syntax fails.
- **Hosts Management:** Appends and cleans up `127.0.0.1 <domain>` entries in `/etc/hosts`.
- **Customizable PHP-FPM:** Supports specifying PHP-FPM versions via `--php` (defaults to `8.3`).

## Installation

```bash
git clone [https://github.com/Wishprogram01/zw-vhost.git](https://github.com/Wishprogram01/zw-vhost.git)
cd zw-vhost
cargo build --release
sudo cp target/release/zw-vhost /usr/local/bin/zw-vhost

