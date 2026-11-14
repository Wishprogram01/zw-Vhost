use clap::{Parser, Subcommand};
use colored::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, exit};

#[derive(Parser)]
#[command(name = "zw-vhost")]
#[command(version = "1.0.0")]
#[command(about = "Local Nginx Virtual Host Manager for Ubuntu/WSL by ZWDevs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Cipta virtual host Nginx baru dan kemas kini /etc/hosts
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
    /// Padam virtual host dan buang entri dari /etc/hosts
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
    println!("  {} {}\n", "Local Nginx Virtual Host Manager".white().bold(), "v1.0.0".bright_black());
}

fn check_root() {
    let output = Command::new("id").arg("-u").output().expect("Gagal semak user ID");
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uid != "0" {
        eprintln!("{} Sila jalankan arahan ini menggunakan {}", "[RALAT]".red().bold(), "sudo".yellow().bold());
        exit(1);
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

    let available_path = format!("/etc/nginx/sites-available/{}", domain);
    let enabled_path = format!("/etc/nginx/sites-enabled/{}", domain);

    if Path::new(&available_path).exists() {
        eprintln!("{} Virtual host untuk '{}' sudah wujud!", "[!]".yellow().bold(), domain);
        exit(1);
    }

    // 1. Tulis fail sites-available
    let config_content = generate_nginx_config(domain, root, php);
    if let Err(e) = fs::write(&available_path, config_content) {
        eprintln!("{} Gagal menulis config Nginx: {}", "[✘]".red().bold(), e);
        exit(1);
    }
    println!("{} Fail config dicipta di {}", "[✔]".green().bold(), available_path.cyan());

    // 2. Cipta symlink ke sites-enabled
    if let Err(e) = symlink(&available_path, &enabled_path) {
        eprintln!("{} Gagal mencipta symlink: {}", "[✘]".red().bold(), e);
        let _ = fs::remove_file(&available_path);
        exit(1);
    }
    println!("{} Symlink diaktifkan di {}", "[✔]".green().bold(), enabled_path.cyan());

    // 3. Semak & Reload Nginx (Auto-rollback jika sintaks gagal)
    if !reload_nginx() {
        eprintln!("{} Konfigurasi Nginx gagal ujian sintaks! Melakukan rollback...", "[✘]".red().bold());
        let _ = fs::remove_file(&enabled_path);
        let _ = fs::remove_file(&available_path);
        exit(1);
    }
    println!("{} Konfigurasi Nginx disahkan & reloaded", "[✔]".green().bold());

    // 4. Kemas kini /etc/hosts
    let hosts_content = fs::read_to_string("/etc/hosts").unwrap_or_default();
    let entry = format!("127.0.0.1\t{}", domain);
    if !hosts_content.contains(domain) {
        let mut file = OpenOptions::new()
            .append(true)
            .open("/etc/hosts")
            .expect("Gagal membuka /etc/hosts");
        writeln!(file, "{}", entry).expect("Gagal menulis ke /etc/hosts");
        println!("{} Entri '{}' ditambah ke /etc/hosts", "[✔]".green().bold(), entry.cyan());
    } else {
        println!("{} Domain sudah wujud dalam /etc/hosts", "[i]".blue().bold());
    }

    println!("\n{} Selesai! Layari projek anda di: {}", "🚀".green(), format!("http://{}", domain).bright_green().underline());
}

fn list_vhosts() {
    println!("{}", "Senarai Virtual Host Aktif (/etc/nginx/sites-enabled):".white().bold());
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

    let available_path = format!("/etc/nginx/sites-available/{}", domain);
    let enabled_path = format!("/etc/nginx/sites-enabled/{}", domain);

    if Path::new(&enabled_path).exists() {
        let _ = fs::remove_file(&enabled_path);
        println!("{} Symlink {} dipadam", "[✔]".green().bold(), enabled_path.cyan());
    }

    if Path::new(&available_path).exists() {
        let _ = fs::remove_file(&available_path);
        println!("{} Fail config {} dipadam", "[✔]".green().bold(), available_path.cyan());
    }

    // Buang baris berkaitan domain daripada /etc/hosts
    if let Ok(hosts_content) = fs::read_to_string("/etc/hosts") {
        let filtered: Vec<&str> = hosts_content
            .lines()
            .filter(|line| !line.split_whitespace().any(|word| word == domain))
            .collect();
        let new_content = format!("{}\n", filtered.join("\n"));
        if fs::write("/etc/hosts", new_content).is_ok() {
            println!("{} Entri '{}' dibuang daripada /etc/hosts", "[✔]".green().bold(), domain.cyan());
        }
    }

    if reload_nginx() {
        println!("{} Nginx reloaded. Virtual host '{}' berjaya dipadam.", "[✔]".green().bold(), domain.yellow());
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
