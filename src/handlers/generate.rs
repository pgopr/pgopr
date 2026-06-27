/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::workload::{DeploymentConfig, PG18_PRIMARY_IMAGE, PG18_REPLICA_IMAGE};
use crate::{crd, persistent, primary, replica, services};
use clap::ArgMatches;
use std::fs;

/// Handles the 'generate' subcommand logic for various resource types.
///
/// # Arguments:
/// - `sub_matches` - The arguments passed to the generate subcommand.
pub fn handle_generate(sub_matches: &ArgMatches) {
    match sub_matches.get_one::<String>("type").unwrap().as_str() {
        "crd" => {
            crd::crd_generate();
        }
        "service" => {
            let s = services::build("postgresql", "default", 5432);
            let data = serde_yaml::to_string(&s).expect("Can't serialize pgopr-service.yaml");
            fs::write("pgopr-service.yaml", data)
                .expect("Unable to write file: pgopr-service.yaml");
        }
        "persistent" => {
            let pv = persistent::build_pv(
                "postgresql-pv-volume",
                5,
                "postgresql",
                "/tmp/kind",
                "postgresql",
            );
            let data = serde_yaml::to_string(&pv).expect("Can't serialize pgopr-pv.yaml");
            fs::write("pgopr-pv.yaml", data).expect("Unable to write file: pgopr-pv.yaml");

            let pvc = persistent::build_pvc("postgresql-pv-claim", "default", 5, "postgresql");
            let data = serde_yaml::to_string(&pvc).expect("Can't serialize pgopr-pvc.yaml");
            fs::write("pgopr-pvc.yaml", data).expect("Unable to write file: pgopr-pvc.yaml");
        }
        "primary" => {
            let p = primary::build(
                "postgresql",
                "default",
                DeploymentConfig {
                    image: PG18_PRIMARY_IMAGE,
                    resources: None,
                    config_map_name: None,
                    config_hash: None,
                },
            );
            let data = serde_yaml::to_string(&p).expect("Can't serialize pgopr-primary.yaml");
            fs::write("pgopr-primary.yaml", data)
                .expect("Unable to write file: pgopr-primary.yaml");
        }
        "replica" => {
            let r = replica::build(
                "postgresql-replica",
                "postgresql",
                "default",
                "replica1",
                DeploymentConfig {
                    image: PG18_REPLICA_IMAGE,
                    resources: None,
                    config_map_name: None,
                    config_hash: None,
                },
            );
            let data = serde_yaml::to_string(&r).expect("Can't serialize pgopr-replica.yaml");
            fs::write("pgopr-replica.yaml", data)
                .expect("Unable to write file: pgopr-replica.yaml");
        }
        "pgexporter" => {
            crate::pgexporter::pgexporter_generate();
        }
        "pgexporter-mon" => {
            let m = crate::pgexporter::build_monitoring_deployment(
                "postgresql-pgexporter-mon",
                "default",
                "postgresql-pgexporter",
                None,
            );
            let data = serde_yaml::to_string(&m).expect("Can't serialize pgexporter-mon.yaml");
            fs::write("pgexporter-mon.yaml", data)
                .expect("Unable to write file: pgexporter-mon.yaml");
        }
        name => {
            unreachable!("Unsupported type `{}`", name)
        }
    }
}
