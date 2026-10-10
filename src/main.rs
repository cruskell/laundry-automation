mod api;
mod auth;
mod decoder;
mod extract;
mod forensics;
mod packed;
mod source_audit;
mod start;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Read and decode Panasonic SmartLaundry state")]
struct Cli {
    #[arg(long, global = true)]
    appliance_id: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List registered devices and save the response locally.
    Devices,
    /// Fetch model-specific property labels and enumeration metadata.
    Info,
    /// Fetch official read-only catalogs for protocol analysis.
    Catalogs,
    /// Compare official property metadata across published communication models.
    ExploreModels,
    /// Read model-supported properties directly from the appliance (command type 03).
    LiveRead,
    /// Probe captured unresolved IDs using read-only command type 03.
    ProbeUnmapped {
        #[arg(long)]
        id: Option<String>,
    },
    /// Audit property references in an official source map and model metadata.
    SourceAudit {
        #[arg(long, default_value = ".local/app.js.map")]
        map: PathBuf,
        #[arg(long, default_value = ".local/models")]
        models: PathBuf,
    },
    /// Fetch appliance status and save raw and decoded responses.
    Status,
    /// Generate an offline immediate-start request without sending it.
    PlanStart,
    /// Start normal wash-and-dry using live settings; dry-run performs reads only.
    Start {
        #[arg(long)]
        dry_run: bool,
    },
    /// Inspect unresolved bytes and compare them with a saved baseline.
    Inspect {
        #[arg(long)]
        snapshot: Option<PathBuf>,
        #[arg(long)]
        baseline: Option<PathBuf>,
    },
    /// Correlate archived raw property transitions with phase and server prediction.
    Timeline {
        #[arg(long, default_value = ".local/snapshots")]
        snapshots: PathBuf,
    },
    /// Decode a saved snapshot without network access.
    Decode {
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
    /// Run the loopback OAuth PKCE callback bridge.
    LoginBridge {
        #[arg(long, default_value_t = 18765)]
        port: u16,
    },
    /// Regenerate the property dictionary from downloaded official data.
    ExtractMap {
        #[arg(long, default_value = ".local/device-info.json")]
        info: PathBuf,
        #[arg(long, default_value = ".local/app.js")]
        bundle: PathBuf,
        #[arg(long, default_value = "spec/property-map.json")]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Devices => api::run("devices", cli.appliance_id.as_deref()),
        Command::Info => api::run("info", cli.appliance_id.as_deref()),
        Command::Catalogs => api::run("catalogs", cli.appliance_id.as_deref()),
        Command::ExploreModels => api::run("explore-models", cli.appliance_id.as_deref()),
        Command::LiveRead => api::run("live-read", cli.appliance_id.as_deref()),
        Command::ProbeUnmapped { id } => api::probe(cli.appliance_id.as_deref(), id.as_deref()),
        Command::SourceAudit { map, models } => source_audit::run(&map, &models),
        Command::Status => api::run("status", cli.appliance_id.as_deref()),
        Command::PlanStart => start::run(cli.appliance_id.as_deref()),
        Command::Start { dry_run } => start::send(cli.appliance_id.as_deref(), dry_run),
        Command::Inspect { snapshot, baseline } => {
            forensics::run(snapshot.as_deref(), baseline.as_deref())
        }
        Command::Timeline { snapshots } => forensics::timeline(&snapshots),
        Command::Decode { snapshot } => {
            let path = snapshot.unwrap_or_else(|| storage::root().join("status.json"));
            let result = decoder::decode(&storage::read(&path)?)?;
            storage::save("decoded-status.json", &result)?;
            api::print_decoded(&result)
        }
        Command::LoginBridge { port } => auth::run(port),
        Command::ExtractMap {
            info,
            bundle,
            output,
        } => extract::run(&info, &bundle, &output),
    }
}
