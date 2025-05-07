/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use clap::{crate_description, crate_name, crate_version, value_parser, Arg, Command};
use clap_complete::{generate, Generator, Shell};
use futures::stream::StreamExt;
use kube::{
    api::Api,
    client::Client,
    runtime::{controller::Action, watcher, Controller},
    Resource, ResourceExt,
};
use log::{debug, error, info, LevelFilter};
use log4rs::{
    append::console::{ConsoleAppender, Target},
    config::{Appender, Config, Logger, Root},
};
use std::sync::Arc;
use tokio::time::Duration;

use crate::crd::v1::pgopr;

pub mod crd;
mod finalizer;
mod k8s;
mod persistent;
mod primary;
mod services;

/// Context injected with each `reconcile` and `on_error` method invocation
struct ContextData {
    /// Kubernetes client
    client: Client,
}

impl ContextData {
    /// Constructs a new instance of ContextData
    ///
    /// # Arguments:
    /// - `client`: Kubernetes client
    pub fn new(client: Client) -> Self {
        ContextData { client }
    }
}

/// Action to be taken upon a `pgopr` resource during reconciliation
enum PgOprAction {
    /// Create the primary subresources
    CreatePrimary,
    /// Delete all primary subresources
    DeletePrimary,
    /// This `PgOpr` resource is in desired state and requires no actions to be taken
    NoOp,
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
                        .value_parser(vec!["crd", "service", "persistent", "primary"])
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
fn generate_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
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
                generate_completions(generator.clone(), &mut cli);
            }
        }

        Some(("generate", sub_matches)) => match *sub_matches.get_one("type").unwrap() {
            "crd" => {
                crd::crd_generate();
            }

            "service" => {
                services::service_generate();
            }

            "persistent" => {
                persistent::persistent_generate();
            }

            "primary" => {
                primary::primary_generate();
            }

            name => {
                unreachable!("Unsupported type `{}`", name)
            }
        },

        Some(("install", _sub_matches)) => {
            info!("{} {}", crate_name!(), crate_version!());
            info!("{}", crate_description!());

            let client: Client = k8s::k8s_client().await;
            let _ = crd::crd_deploy(client).await;
        }

        Some(("provision", sub_matches)) => {
            let provision_command = sub_matches.subcommand().unwrap_or(("primary", sub_matches));
            match provision_command {
                ("primary", _sub_matches) => {
                    info!("{} {}", crate_name!(), crate_version!());
                    info!("{}", crate_description!());

                    debug!("primary");
                    let client: Client = k8s::k8s_client().await;
                    let namespace = "default".to_owned();

                    let _pv = persistent::persistent_volume_deploy(
                        client.clone(),
                        "postgresql-pv-volume",
                        5u32,
                    )
                    .await;

                    let _pvc = persistent::persistent_volume_claim_deploy(
                        client.clone(),
                        "postgresql-pv-claim",
                        &namespace,
                        5u32,
                    )
                    .await;

                    let _d =
                        primary::primary_deploy(client.clone(), "postgresql", &namespace).await;

                    let _s =
                        services::service_deploy(client.clone(), "postgresql", &namespace).await;
                }

                (name, _) => {
                    unreachable!("Unsupported subcommand `{}`", name)
                }
            }
        }

        Some(("retire", sub_matches)) => {
            let retire_command = sub_matches.subcommand().unwrap_or(("primary", sub_matches));
            match retire_command {
                ("primary", _sub_matches) => {
                    info!("{} {}", crate_name!(), crate_version!());
                    info!("{}", crate_description!());

                    debug!("primary");
                    let client: Client = k8s::k8s_client().await;
                    let namespace = "default".to_owned();

                    let _s =
                        services::service_undeploy(client.clone(), "postgresql", &namespace).await;
                    let _d =
                        primary::primary_undeploy(client.clone(), "postgresql", &namespace).await;
                    let _pvc = persistent::persistent_volume_claim_undeploy(
                        client.clone(),
                        "postgresql-pv-claim",
                        &namespace,
                    )
                    .await;
                    let _pv = persistent::persistent_volume_undeploy(
                        client.clone(),
                        "postgresql-pv-volume",
                    )
                    .await;
                }
                (name, _) => {
                    unreachable!("Unsupported subcommand `{}`", name)
                }
            }
        }

        Some(("uninstall", _sub_matches)) => {
            info!("{} {}", crate_name!(), crate_version!());
            info!("{}", crate_description!());

            let client: Client = k8s::k8s_client().await;
            let _ = crd::crd_undeploy(client).await;
        }

        _ => {
            info!("{} {}", crate_name!(), crate_version!());
            info!("{}", crate_description!());

            let client: Client = k8s::k8s_client().await;
            let crd_api: Api<pgopr> = Api::all(client.clone());
            let context: Arc<ContextData> = Arc::new(ContextData::new(client.clone()));

            // Start the controller
            Controller::new(crd_api.clone(), watcher::Config::default())
                .run(reconcile, on_error, context)
                .for_each(|reconciliation_result| async move {
                    match reconciliation_result {
                        Ok(pgopr_resource) => {
                            debug!("Reconciliation successful. Resource: {:?}", pgopr_resource);
                        }
                        Err(reconciliation_err) => {
                            error!("Reconciliation error: {:?}", reconciliation_err)
                        }
                    }
                })
                .await;
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
    let namespace: String = match pgopr.namespace() {
        None => {
            return Err(Error::UserInputError(
                "Expected pgopr resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        }

        Some(namespace) => namespace,
    };

    // Performs action as decided by the `determine_action` function.
    return match determine_action(&pgopr) {
        PgOprAction::CreatePrimary => {
            let name = pgopr.name_any();

            finalizer::add(client.clone(), &name, &namespace).await?;
            primary::primary_deploy(client, &pgopr.name_any(), &namespace).await?;

            Ok(Action::requeue(Duration::from_secs(10)))
        }

        PgOprAction::DeletePrimary => {
            primary::primary_undeploy(client.clone(), &pgopr.name_any(), &namespace).await?;
            finalizer::delete(client, &pgopr.name_any(), &namespace).await?;

            Ok(Action::await_change())
        }

        PgOprAction::NoOp => Ok(Action::requeue(Duration::from_secs(10))),
    };
}

/// Determine the action
///
/// # Arguments
/// - `pgopr`: A reference to `pgopr` being reconciled to decide next action upon
///
fn determine_action(pgopr: &pgopr) -> PgOprAction {
    return if pgopr.meta().deletion_timestamp.is_some() {
        PgOprAction::DeletePrimary
    } else if pgopr
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        PgOprAction::CreatePrimary
    } else {
        PgOprAction::NoOp
    };
}

/// The on_error callback
///
/// # Arguments
/// - `error`: The error
/// - `_context`: Unused argument
fn on_error(_obj: Arc<pgopr>, error: &Error, _context: Arc<ContextData>) -> Action {
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
}
