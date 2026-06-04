//! t3unlock: Cross-platform CLI to unlock Samsung Portable SSD T1/T3/T5.
//!
//! Protocol: bulk transfers (NOT control transfers).
//!   - OUT endpoint: 0x02
//!   - IN endpoint:  0x81
//!   - Sequence: unlock(31B) → password(512B) → relink(31B)

mod cli;
mod errors;
mod usb;

use anyhow::Result;
use cli::{Cli, Commands};
use tracing::{error, info};

fn main() {
    if let Err(e) = real_main() {
        error!(error = %e, "fatal error");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("t3unlock=info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.cmd {
        Commands::Status { model, json } => {
            let m = usb::Model::from_str(&model)
                .ok_or_else(|| anyhow::anyhow!("unknown model: {}", model))?;
            let sel = usb::DeviceSelector::new(m);
            let status = usb::status(&sel)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("Model:   {}", status.model);
                println!("VID:     0x{:04x}", status.vid);
                println!("PID:     0x{:04x}", status.pid);
                println!("Present: {}", status.present);
                println!("Locked:  {}", status.locked.unwrap_or(false));
                println!("EP OUT:  0x{:02x}", status.ep_out);
                println!("EP IN:   0x{:02x}", status.ep_in);
            }
        }

        Commands::Unlock { model, password, dry_run, timeout_ms } => {
            let m = usb::Model::from_str(&model)
                .ok_or_else(|| anyhow::anyhow!("unknown model: {}", model))?;
            let sel = usb::DeviceSelector::new(m);

            if dry_run {
                info!(model = %m.label(), "DRY RUN: would perform unlock sequence");
                println!("DRY RUN: unlock sequence simulated");
                return Ok(());
            }

            let pass = password.unwrap_or_else(|| {
                read_password_interactive()
            });

            let res = usb::unlock(&sel, pass.as_bytes(), timeout_ms);

            // Zero the password no matter what
            let mut p = pass.into_bytes();
            zeroize::Zeroize::zeroize(&mut p);

            res?;
            println!("Unlock successful.");
        }

        Commands::Doctor {} => {
            let report = usb::doctor()?;
            println!("{}", report);
        }

        Commands::GenCompletions { shell } => {
            cli::gen_completions(shell)?;
        }

        Commands::GenMan { out } => {
            cli::gen_man(out)?;
        }
    }

    Ok(())
}

/// Read password interactively — tries stdin first, falls back to /dev/tty.
/// This ensures sudo and other scenarios where stdin is not a terminal still work.
fn read_password_interactive() -> String {
    // Try stdin first if it's a tty
    if atty::is(atty::Stream::Stdin) {
        if let Ok(pass) = rpassword::prompt_password("Enter drive password: ") {
            if !pass.is_empty() {
                return pass;
            }
        }
    }

    // Fallback: read from /dev/tty (direct terminal, bypasses sudo stdin redirect)
    if let Ok(fd) = std::fs::File::open("/dev/tty") {
        use std::io::{BufRead, Write};
        let mut tty = fd;
        let stdout = std::io::stdout();
        let _ = write!(stdout, "Enter drive password: ");
        let _ = stdout.flush();
        let mut line = String::new();
        if std::io::BufReader::new(&mut tty).read_line(&mut line).is_ok() {
            return line.trim_end().to_string();
        }
    }

    rpassword::prompt_password("Enter drive password: ")
        .unwrap_or_default()
}
