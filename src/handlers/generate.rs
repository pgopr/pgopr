/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::{crd, persistent, primary, services};
use clap::ArgMatches;

/// Handles the 'generate' subcommand logic for various resource types.
///
/// # Arguments:
/// - `sub_matches` - The arguments passed to the generate subcommand.
pub fn handle_generate(sub_matches: &ArgMatches) {
    match *sub_matches.get_one::<&str>("type").unwrap() {
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
    }
}
