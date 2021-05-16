use std::net::IpAddr;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use huelib::resource::light::{AttributeModifier, Scanner, StateModifier};
use huelib::Bridge;
use prettytable::{cell, format, row, Table};
use serde_derive::Deserialize;
use structopt::StructOpt;

const SCAN_SLEEP_TIME: Duration = Duration::from_secs(40);

#[derive(Debug, Deserialize)]
struct Config {
    username: String,
}

#[derive(Debug, StructOpt)]
enum Subcommand {
    /// Register a new username on a Hue bridge
    Register,

    Scan,

    List,

    Blink {
        id: String,
    },

    Name {
        id: String,
        name: String,
    },
}

#[derive(Debug, StructOpt)]
#[structopt(name = "hue", about = "Helper for Philips Hue lights")]
struct Args {
    #[structopt(subcommand)]
    subcommand: Subcommand,

    /// Optional IP address for a specific bridge. Tool will search the network if no IP is
    /// provided.
    ip_address: Option<IpAddr>,
}

fn cmd_scan(bridge: Bridge) -> Result<()> {
    let scanner = Scanner::new();
    bridge.search_new_lights(&scanner)?;

    println!("Initiated scan. Sleeping for {:?}.", SCAN_SLEEP_TIME);
    std::thread::sleep(SCAN_SLEEP_TIME);

    bridge.get_new_lights()?;

    Ok(())
}

fn cmd_list(bridge: Bridge) -> Result<()> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

    table.set_titles(row!["id", "name", "reachable", "on"]);

    let mut lights = bridge.get_all_lights()?;

    lights.sort_by(|a, b| a.id.cmp(&b.id));

    for light in lights {
        table.add_row(row![
            light.id.to_string(),
            light.name,
            if light.state.reachable {
                "yes".to_string()
            } else {
                "no".to_string()
            },
            light
                .state
                .on
                .map(|on| if on {
                    "yes".to_string()
                } else {
                    "no".to_string()
                })
                .unwrap_or("-".to_string()),
        ]);
    }

    table.printstd();

    Ok(())
}

fn cmd_blink(bridge: Bridge, id: String) -> Result<()> {
    println!("Blinking light {}...", id);

    let mut on = true;
    loop {
        let modifier = StateModifier::new().with_on(on);
        bridge.set_light_state(id.clone(), &modifier)?;

        on = if on { false } else { true };

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn cmd_name(bridge: Bridge, id: String, name: String) -> Result<()> {
    bridge.set_light_attribute(
        id.clone(),
        &AttributeModifier::new().with_name(name.clone()),
    )?;

    println!("Set light {} name to \"{}\"", id, name);

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::from_args();

    let address = match args.ip_address {
        Some(address) => address,
        None => {
            let mut ip_addresses = huelib::bridge::discover_nupnp()?;
            ip_addresses
                .pop()
                .ok_or_else(|| anyhow!("No bridge IP addresses found on the network"))?
        }
    };

    if let Subcommand::Register = args.subcommand {
        let username = huelib::bridge::register_user(address, "hue-rs-cli")?;
        println!("Username: {}", username);

        return Ok(());
    }

    let dirs = xdg::BaseDirectories::with_prefix("hue")?;
    let config_file = dirs
        .find_config_file("config.toml")
        .ok_or_else(|| anyhow!("no hue config file found in .config/hue"))?;
    let config: Config = toml::from_str(
        &std::fs::read_to_string(config_file.clone())
            .with_context(|| anyhow!("failed to read config file at {:?}", config_file))?,
    )
    .with_context(|| anyhow!("failed to parse config file at {:?}", config_file))?;

    let bridge = Bridge::new(address, &config.username);

    match args.subcommand {
        Subcommand::Register => {
            return Err(anyhow!(
                "Somehow managed to call register after loading config. Should not be possible."
            ));
        }
        Subcommand::Scan => cmd_scan(bridge),
        Subcommand::List => cmd_list(bridge),
        Subcommand::Blink { id } => cmd_blink(bridge, id),
        Subcommand::Name { id, name } => cmd_name(bridge, id, name),
    }
}
