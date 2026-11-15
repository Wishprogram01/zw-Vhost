use clap::{Parser, Subcommand};
use colored::*;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{exit, Command};

const LINUX_HOSTS_PATH: &str = "/etc/hosts";
const WIN_HOSTS_PATH: &str = "/mnt/c/Windows/System32/drivers/etc/hosts";

#[derive(Parser)]
#[command(name = "zw-vhost")]
#[command(version = "1.1.0")]
#[command(about = "Local Nginx Virtual Host Manager for Ubuntu/WSL by ZWDevs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Cipta virtual host Nginx baru dan kemas kini /etc/hosts (+ Windows hosts di WSL)
    Create {
        /// Nama domain tempatan (cth: projek2.test)
        domain: String,

        /// Laluan direktori root projek (cth: /var/www/projek2/public)
        #[arg(short, long)]
        root: String,

        /// Versi PHP-FPM (default: 8.3)
        #[arg(short, long, default_value = "8.3")]
        php: String,
    },
    /// Senaraikan semua virtual host yang aktif
    List,
    /// Padam virtual host dan buang entri dari /etc/hosts (+ Windows hosts di WSL)
    Delete {
        /// Nama domain untuk dipadam (cth: projek2.test)
        domain: String,
    },
}

fn print_banner() {
    let banner = r#"
  ███████╗██╗    ██╗      ██╗   ██╗██╗  ██╗ ██████╗ ███████╗████████╗
  ╚══███╔╝██║    ██║      ██║   ██║██║  ██║██╔═══██╗██╔════╝╚══██╔══╝
    ███╔╝ ██║ █╗ ██║█████╗██║   ██║███████║██║   ██║███████╗   ██║   
   ███╔╝  ██║███╗██║╚════╝╚██╗ ██╔╝██╔══██║██║   ██║╚════██║   ██║   
  ███████╗╚███╔███╔╝       ╚████╔╝ ██║  ██║╚██████╔╝███████║   ██║   
  ╚══════╝ ╚══╝╚══╝         ╚═══╝  ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝   
    "#;
    println!("{}", banner.bright_cyan().bold());
    println!(
        "  {} {}\n",
        "Local Nginx Virtual Host Manager".white().bold(),
        "v1.1.0".bright_black()
    );
}

fn check_root() {
    let output = Command::new("id").arg("-u").output().expect("Gagal semak user ID");
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uid != "0" {
        eprintln!(
            "{} Sila jalankan arahan ini menggunakan {}",
            "[RALAT]".red().bold(),
            "sudo".yellow().bold()
        );
        exit(1);
    }
}

fn validate_domain(domain: &str) {
    if domain.is_empty()
        || domain.contains('/')
        || domain.contains('\\')
        || domain.chars().any(char::is_whitespace)
    {
        eprintln!(
            "{} Nama domain '{}' tidak sah.",
            "[RALAT]".red().bold(),
            domain
        );
        exit(1);
    }
}

fn is_wsl_with_win_hosts() -> bool {
    let proc_version = fs::read_to_string("/proc/version")
        .unwrap_or_default()
        .to_lowercase();
    let is_wsl = proc_version.contains("microsoft") || proc_version.contains("wsl");
    is_wsl && Path::new(WIN_HOSTS_PATH).exists()
}

fn domain_exists_in_hosts(content: &str, domain: &str) -> bool {
    content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .any(|line| line.split_whitespace().any(|word| word == domain))
}

fn print_win_hosts_permission_warning(domain: &str, action: &str) {
    eprintln!(
        "{} Akses ke Windows hosts ({}) ditolak (Permission Denied).",
        "[!]".yellow().bold(),
        WIN_HOSTS_PATH.bright_black()
    );
    eprintln!(
        "    Sila buka Windows Terminal sebagai {} untuk auto-sync, atau {} entri '{}' secara manual.",
        "Administrator".yellow().bold(),
        action,
        format!("127.0.0.1 {}", domain).cyan()
    );
}

fn add_host_entry(path: &str, domain: &str, is_windows: bool) {
    let label = if is_windows { "Windows hosts" } else { "/etc/hosts" };

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if is_windows && e.kind() == ErrorKind::PermissionDenied => {
            print_win_hosts_permission_warning(domain, "tambah");
            return;
        }
        Err(e) => {
            eprintln!("{} Gagal membaca {}: {}", "[!]".yellow().bold(), label, e);
            return;
        }
    };

    if domain_exists_in_hosts(&content, domain) {
        println!(
            "{} Domain '{}' sudah wujud dalam {}",
            "[i]".blue().bold(),
            domain,
            label
        );
        return;
    }

    let eol = if is_windows { "\r\n" } else { "\n" };
    let prefix = if !content.is_empty() && !content.ends_with('\n') {
        eol
    } else {
        ""
    };
    let entry = format!("127.0.0.1\t{}", domain);

    match OpenOptions::new().append(true).open(path) {
        Ok(mut file) => {
            if let Err(e) = write!(file, "{}{}{}", prefix, entry, eol) {
                if is_windows && e.kind() == ErrorKind::PermissionDenied {
                    print_win_hosts_permission_warning(domain, "tambah");
                } else {
                    eprintln!("{} Gagal menulis ke {}: {}", "[!]".yellow().bold(), label, e);
                }
            } else {
                println!(
                    "{} Entri '{}' ditambah ke {}",
                    "[✔]".green().bold(),
                    entry.cyan(),
                    label
                );
            }
        }
        Err(e) if is_windows && e.kind() == ErrorKind::PermissionDenied => {
            print_win_hosts_permission_warning(domain, "tambah");
        }
        Err(e) => {
            eprintln!("{} Gagal membuka {}: {}", "[!]".yellow().bold(), label, e);
        }
    }
}

fn remove_host_entry(path: &str, domain: &str, is_windows: bool) {
    let label = if is_windows { "Windows hosts" } else { "/etc/hosts" };

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if is_windows && e.kind() == ErrorKind::PermissionDenied => {
            print_win_hosts_permission_warning(domain, "buang");
            return;
        }
        Err(e) => {
            eprintln!("{} Gagal membaca {}: {}", "[!]".yellow().bold(), label, e);
            return;
        }
    };

    let original_lines: Vec<&str> = content.lines().collect();
    let filtered: Vec<&str> = original_lines
        .iter()
        .copied()
        .filter(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                return true;
            }
            !trimmed.split_whitespace().any(|word| word == domain)
        })
        .collect();

    if filtered.len() == original_lines.len() {
        return;
    }

    let eol = if is_windows { "\r\n" } else { "\n" };
    let new_content = if filtered.is_empty() {
        String::new()
    } else {
        format!("{}{}", filtered.join(eol), eol)
    };

    match fs::write(path, new_content) {
        Ok(_) => {
            println!(
                "{} Entri '{}' dibuang daripada {}",
                "[✔]".green().bold(),
                domain.cyan(),
                label
            );
        }
        Err(e) if is_windows && e.kind() == ErrorKind::PermissionDenied => {
            print_win_hosts_permission_warning(domain, "buang");
        }
        Err(e) => {
            eprintln!("{} Gagal mengemaskini {}: {}", "[!]".yellow().bold(), label, e);
        }
    }
}

fn generate_nginx_config(domain: &str, root: &str, php_version: &str) -> String {
    format!(
        r#"server {{
    listen 80;
    listen [::]:80;

    server_name {domain};
    root {root};

    index index.php index.html index.htm;

    location / {{
        try_files $uri $uri/ /index.php?$query_string;
    }}

    location ~ \.php$ {{
        include snippets/fastcgi-php.conf;
        fastcgi_pass unix:/var/run/php/php{php_version}-fpm.sock;
    }}

    location ~ /\.ht {{
        deny all;
    }}
}}
"#
    )
}

fn reload_nginx() -> bool {
    let test = Command::new("nginx").arg("-t").output();
    match test {
        Ok(output) if output.status.success() => {
            let reload = Command::new("service").args(["nginx", "reload"]).status();
            reload.map(|s| s.success()).unwrap_or(false)
        }
        Ok(output) => {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr).red());
            false
        }
        Err(_) => false,
    }
}

fn create_vhost(domain: &str, root: &str, php: &str) {
    check_root();
    validate_domain(domain);

    if !Path::new(root).exists() {
        println!(
            "{} Amaran: Direktori root '{}' belum wujud lagi.",
            "[!]".yellow().bold(),
            root.yellow()
        );
    }

    let available_path = format!("/etc/nginx/sites-available/{}", domain);
    let enabled_path = format!("/etc/nginx/sites-enabled/{}", domain);

    if Path::new(&available_path).exists() {
        eprintln!(
            "{} Virtual host untuk '{}' sudah wujud!",
            "[!]".yellow().bold(),
            domain
        );
        exit(1);
    }

    // 1. Tulis fail sites-available
    let config_content = generate_nginx_config(domain, root, php);
    if let Err(e) = fs::write(&available_path, config_content) {
        eprintln!("{} Gagal menulis config Nginx: {}", "[✘]".red().bold(), e);
        exit(1);
    }
    println!(
        "{} Fail config dicipta di {}",
        "[✔]".green().bold(),
        available_path.cyan()
    );

    // 2. Cipta symlink ke sites-enabled
    if let Err(e) = symlink(&available_path, &enabled_path) {
        eprintln!("{} Gagal mencipta symlink: {}", "[✘]".red().bold(), e);
        let _ = fs::remove_file(&available_path);
        exit(1);
    }
    println!(
        "{} Symlink diaktifkan di {}",
        "[✔]".green().bold(),
        enabled_path.cyan()
    );

    // 3. Semak & Reload Nginx (Auto-rollback jika sintaks gagal)
    if !reload_nginx() {
        eprintln!(
            "{} Konfigurasi Nginx gagal ujian sintaks! Melakukan rollback...",
            "[✘]".red().bold()
        );
        let _ = fs::remove_file(&enabled_path);
        let _ = fs::remove_file(&available_path);
        exit(1);
    }
    println!(
        "{} Konfigurasi Nginx disahkan & reloaded",
        "[✔]".green().bold()
    );

    // 4. Kemas kini /etc/hosts & Windows hosts (jika WSL)
    add_host_entry(LINUX_HOSTS_PATH, domain, false);
    if is_wsl_with_win_hosts() {
        add_host_entry(WIN_HOSTS_PATH, domain, true);
    }

    println!(
        "\n{} Selesai! Layari projek anda di: {}",
        "🚀".green(),
        format!("http://{}", domain).bright_green().underline()
    );
}

fn list_vhosts() {
    println!(
        "{}",
        "Senarai Virtual Host Aktif (/etc/nginx/sites-enabled):"
            .white()
            .bold()
    );
    match fs::read_dir("/etc/nginx/sites-enabled") {
        Ok(entries) => {
            let mut count = 0;
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                println!("  {} {}", "•".bright_cyan(), name.green());
                count += 1;
            }
            if count == 0 {
                println!("  {}", "Tiada virtual host aktif ditemui.".bright_black());
            }
        }
        Err(e) => eprintln!("{} Gagal membaca direktori: {}", "[✘]".red().bold(), e),
    }
}

fn delete_vhost(domain: &str) {
    check_root();
    validate_domain(domain);

    let available_path = format!("/etc/nginx/sites-available/{}", domain);
    let enabled_path = format!("/etc/nginx/sites-enabled/{}", domain);
    let mut removed_any = false;

    if Path::new(&enabled_path).symlink_metadata().is_ok() {
        if let Err(e) = fs::remove_file(&enabled_path) {
            eprintln!("{} Gagal memadam symlink: {}", "[✘]".red().bold(), e);
        } else {
            println!("{} Symlink {} dipadam", "[✔]".green().bold(), enabled_path.cyan());
            removed_any = true;
        }
    }

    if Path::new(&available_path).exists() {
        if let Err(e) = fs::remove_file(&available_path) {
            eprintln!("{} Gagal memadam config: {}", "[✘]".red().bold(), e);
        } else {
            println!(
                "{} Fail config {} dipadam",
                "[✔]".green().bold(),
                available_path.cyan()
            );
            removed_any = true;
        }
    }

    if !removed_any {
        println!(
            "{} Tiada fail konfigurasi Nginx ditemui untuk '{}'.",
            "[i]".blue().bold(),
            domain
        );
    }

    // Buang entri daripada /etc/hosts & Windows hosts (jika WSL)
    remove_host_entry(LINUX_HOSTS_PATH, domain, false);
    if is_wsl_with_win_hosts() {
        remove_host_entry(WIN_HOSTS_PATH, domain, true);
    }

    if reload_nginx() {
        println!(
            "{} Nginx reloaded. Virtual host '{}' berjaya dipadam.",
            "[✔]".green().bold(),
            domain.yellow()
        );
    } else {
        eprintln!(
            "{} Nginx gagal reload selepas memadam '{}'. Semak 'sudo nginx -t'.",
            "[✘]".red().bold(),
            domain
        );
    }
}

fn main() {
    let cli = Cli::parse();
    print_banner();

    match &cli.command {
        Commands::Create { domain, root, php } => create_vhost(domain, root, php),
        Commands::List => list_vhosts(),
        Commands::Delete { domain } => delete_vhost(domain),
    }
}
