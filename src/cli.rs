use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "t3unlock", version, about = "Unlock Samsung Portable SSD T1/T3/T5")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show device presence and lock status
    Status {
        /// Device model: t1, t3, or t5
        #[arg(value_name = "MODEL", default_value = "t3")]
        model: String,
        /// Output JSON
        #[arg(long)]
        json: bool,
    },
    /// Unlock the drive (prompts for password if not provided)
    Unlock {
        /// Device model: t1, t3, or t5
        #[arg(value_name = "MODEL", default_value = "t3")]
        model: String,
        /// Password (unsafe on shared shells; prefer interactive prompt)
        #[arg(long)]
        password: Option<String>,
        /// Simulate the unlock sequence without touching USB
        #[arg(long)]
        dry_run: bool,
        /// USB transfer timeout in milliseconds (default 3000)
        #[arg(long)]
        timeout_ms: Option<u64>,
    },
    /// Diagnose common permission/connection issues
    Doctor {},
    /// Generate shell completions
    GenCompletions {
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    /// Generate a man page to the given directory
    GenMan {
        /// Output directory for man page (e.g., ./target)
        out: PathBuf,
    },
}

#[derive(Clone, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

impl Cli {
    pub fn parse() -> Self {
        <Cli as Parser>::parse()
    }
}

pub fn gen_completions(shell: CompletionShell) -> Result<()> {
    use clap_complete::{generate, shells};
    use std::io;

    let mut cmd = Cli::command();
    match shell {
        CompletionShell::Bash => generate(shells::Bash, &mut cmd, "t3unlock", &mut io::stdout()),
        CompletionShell::Zsh => generate(shells::Zsh, &mut cmd, "t3unlock", &mut io::stdout()),
        CompletionShell::Fish => generate(shells::Fish, &mut cmd, "t3unlock", &mut io::stdout()),
    }
    Ok(())
}

pub fn gen_man(out: std::path::PathBuf) -> Result<()> {
    use clap::CommandFactory;
    use clap_mangen::Man;
    use std::fs;
    fs::create_dir_all(&out)?;
    let man = Man::new(Cli::command());
    let mut buf: Vec<u8> = Default::default();
    man.render(&mut buf)?;
    let path = out.join("t3unlock.1");
    std::fs::write(&path, buf)?;
    eprintln!("Wrote {}", path.display());
    Ok(())
}
