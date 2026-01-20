/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

pub mod cluster;
pub mod generate;
pub mod operator;

use clap::{crate_description, crate_name, crate_version};
use log::info;

// Helper to remove that repeated block from main.rs
pub fn print_header() {
    info!("{} {}", crate_name!(), crate_version!());
    info!("{}", crate_description!());
}
