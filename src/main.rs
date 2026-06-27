/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use clap::{Arg, Command, crate_description, crate_name, crate_version, value_parser};
use clap_complete::{Generator, Shell, generate};
use kube::{Resource, ResourceExt, client::Client, runtime::controller::Action};
use log::LevelFilter;
use log4rs::{
    append::console::{ConsoleAppender, Target},
    config::{Appender, Config, Logger, Root},
};
use std::sync::Arc;
use tokio::time::Duration;

use crate::crd::v1::pgopr;

mod cluster;
pub mod crd;
mod finalizer;
pub mod handlers;
mod k8s;
mod manager;
mod persistent;
mod pgexporter;
mod pgmoneta;
mod primary;
mod replica;
mod services;
mod workload;

/// Context injected with each `reconcile` and `on_error` method invocation
pub(crate) struct ContextData {
    /// Kubernetes client
    client: Client,
}

impl ContextData {
    pub fn new(client: Client) -> Self {
        ContextData { client }
    }
}

/// Initialize the logging frameworks
fn init_log() {
    let console = ConsoleAppender::builder().target(Target::Stdout).build();
    let config = Config::builder()
        .appender(Appender::builder().build("console", Box::new(console)))
        .logger(Logger::builder().build("pgopr", LevelFilter::Info))
        .build(Root::builder().appender("console").build(LevelFilter::Info))
        .unwrap();
    let _handle = log4rs::init_config(config).unwrap();
}

/// Parse main arguments into a Command instance
fn cli() -> Command {
    Command::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .propagate_version(true)
        .trailing_var_arg(true)
        .after_help(
            "pgopr: https://pgopr.github.io/\nReport bugs: https://github.com/pgopr/pgopr/issues",
        )
        .subcommand(
            Command::new("install")
                .about("Install the operator")
                .display_order(1),
        )
        .subcommand(
            Command::new("provision")
                .about("Provision a component")
                .display_order(2)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("primary")
                        .about("Provision a primary instance")
                        .display_order(1),
                )
                .subcommand(
                    Command::new("replica")
                        .about("Provision a replica instance")
                        .display_order(2),
                )
                .subcommand(
                    Command::new("pgmoneta")
                        .about("Provision a pgmoneta instance")
                        .display_order(3),
                )
                .subcommand(
                    Command::new("pgexporter")
                        .about("Provision a pgexporter instance")
                        .display_order(4),
                )
                .subcommand(
                    Command::new("grafana")
                        .about("Provision a pgexporter monitoring instance (Grafana + Prometheus)")
                        .display_order(5),
                ),
        )
        .subcommand(
            Command::new("retire")
                .about("Retire a component")
                .display_order(3)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("primary")
                        .about("Retire a primary instance")
                        .display_order(1),
                )
                .subcommand(
                    Command::new("replica")
                        .about("Retire a replica instance")
                        .display_order(2),
                )
                .subcommand(
                    Command::new("pgmoneta")
                        .about("Retire a pgmoneta instance")
                        .display_order(3),
                )
                .subcommand(
                    Command::new("pgexporter")
                        .about("Retire a pgexporter instance")
                        .display_order(4),
                )
                .subcommand(
                    Command::new("grafana")
                        .about("Retire a pgexporter monitoring instance (Grafana + Prometheus)")
                        .display_order(5),
                ),
        )
        .subcommand(
            Command::new("uninstall")
                .about("Uninstall the operator")
                .display_order(4),
        )
        .subcommand(
            Command::new("completion")
                .about("Generate a shell completion file")
                .display_order(997)
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .required(true)
                        .value_parser(value_parser!(Shell))
                        .help("Generate a shell completion file"),
                ),
        )
        .subcommand(
            Command::new("generate")
                .about("Generate YAML resources")
                .display_order(998)
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .required(true)
                        .value_parser(vec![
                            "crd",
                            "service",
                            "persistent",
                            "primary",
                            "replica",
                            "pgexporter",
                            "pgexporter-mon",
                        ])
                        .help("Generate YAML resources"),
                ),
        )
}

/// Generate shell completion templates
///
/// # Arguments:
/// - `gen` - The generator to be used
/// - `cmd` - The command line structure
///
fn generate_completions<G: Generator>(r#gen: G, cmd: &mut Command) {
    generate(
        r#gen,
        cmd,
        cmd.get_name().to_string(),
        &mut std::io::stdout(),
    );
}

/// The main method
#[tokio::main]
async fn main() {
    let clicmd = cli().get_matches();

    init_log();

    match clicmd.subcommand() {
        Some(("completion", sub_matches)) => {
            if let Some(generator) = sub_matches.get_one::<Shell>("type") {
                let mut cli = cli();
                generate_completions(*generator, &mut cli);
            }
        }

        Some(("generate", sub_matches)) => {
            handlers::generate::handle_generate(sub_matches);
        }

        Some(("install", _)) => {
            handlers::cluster::handle_install().await;
        }

        Some(("provision", sub_matches)) => {
            let (name, _) = sub_matches.subcommand().unwrap_or(("primary", sub_matches));
            match name {
                "primary" => handlers::cluster::handle_provision_primary().await,
                "replica" => handlers::cluster::handle_provision_replica().await,
                "pgmoneta" => handlers::cluster::handle_provision_pgmoneta().await,
                "pgexporter" => handlers::cluster::handle_provision_pgexporter().await,
                "grafana" => handlers::cluster::handle_provision_grafana().await,
                name => unreachable!("Unsupported subcommand `{}`", name),
            }
        }

        Some(("retire", sub_matches)) => {
            let (name, _) = sub_matches.subcommand().unwrap_or(("primary", sub_matches));
            match name {
                "primary" => handlers::cluster::handle_retire_primary().await,
                "replica" => handlers::cluster::handle_retire_replica().await,
                "pgmoneta" => handlers::cluster::handle_retire_pgmoneta().await,
                "pgexporter" => handlers::cluster::handle_retire_pgexporter().await,
                "grafana" => handlers::cluster::handle_retire_grafana().await,
                name => unreachable!("Unsupported subcommand `{}`", name),
            }
        }

        Some(("uninstall", _)) => {
            handlers::cluster::handle_uninstall().await;
        }

        _ => {
            handlers::operator::run_operator().await;
        }
    }
}

/// Reconcile
///
/// # Arguments:
/// - `pgopr` - The pgopr resource
/// - `context` - The context
///
async fn reconcile(pgopr: Arc<pgopr>, context: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = context.client.clone();
    let cluster = crate::cluster::Cluster::new(client.clone());
    let namespace = pgopr.namespace().unwrap_or("default".into());
    let name = pgopr.name_any();

    if pgopr.meta().deletion_timestamp.is_some() {
        cluster.cleanup_all(&pgopr).await?;
        finalizer::delete(client, &name, &namespace).await?;
        return Ok(Action::await_change());
    }

    if pgopr
        .meta()
        .finalizers
        .as_ref()
        .is_none_or(|finalizers| finalizers.is_empty())
    {
        finalizer::add(client.clone(), &name, &namespace).await?;
    }

    // sync with the cluster manager and update status
    cluster.reconcile_state(pgopr.clone()).await?;

    Ok(Action::requeue(Duration::from_secs(30)))
}
/// The on_error callback
///
/// # Arguments
/// - `error`: The error
/// - `_context`: Unused argument
pub(crate) fn on_error(_obj: Arc<pgopr>, error: &Error, _context: Arc<ContextData>) -> Action {
    eprintln!("Reconciliation error:\n{:?}", error);
    Action::requeue(Duration::from_secs(5))
}

/// All errors possible to occur during reconciliation
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Any error originating from the `kube-rs` crate
    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },
    /// Error in user input or pgopr resource definition, typically missing fields.
    #[error("Invalid pgopr CRD: {0}")]
    UserInputError(String),

    /// Error on unsupported PostgreSQL version
    #[error("Unsupported PostgreSQL version: {0}")]
    UnsupportedPostgresVersion(String),
}
