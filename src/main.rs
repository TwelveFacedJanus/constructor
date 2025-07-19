mod config;
mod builder;

use anyhow::Result;
use clap::Parser;
use log::info;

use crate::builder::DefaultBuilder;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value="WORKSPACE.constructor")]
    config: String,

    #[arg(long)]
    clean: bool,

    #[arg(long)]
    force: bool,
}

fn main() -> Result<()>
{
    simple_logger::init_with_level(log::Level::Info)?;
    let args = Args::parse();

    if args.clean {
        info!("Cleaning build artifacts...");
        let config = config::load_config(&args.config)?;
        let builder = builder::Builder::new(config, args.force);
        builder.clean_cache()?;
        return Ok(());
    }

    info!("Loading configuration from {}...", args.config);
    let config = config::load_config(&args.config)?;

    info!("Loading configuration from {}...", args.config);
    let builder = builder::Builder::new(config, args.force);
    builder.build()?;

    info!("Build completed successfully!");
    Ok(())
}
