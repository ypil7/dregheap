use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about = None, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub port: u16,
}
