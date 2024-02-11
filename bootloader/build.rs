mod build_helpers;

use crate::build_helpers::x86_64;

fn main() {
    #[cfg(feature = "bios")]
    x86_64::build_bios();
}
