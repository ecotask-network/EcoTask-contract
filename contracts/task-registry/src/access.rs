use crate::storage;
use soroban_sdk::{Address, Env};

/// Requires the caller to be the admin address.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to check against the stored admin
///
/// # Panics
///
/// Panics with "registry: unauthorized" if the address is not the admin.
pub fn require_admin(e: &Env, addr: &Address) {
    let admin = storage::read_admin(e);
    if addr != &admin {
        panic!("registry: unauthorized");
    }
}

/// Requires the caller to be either the admin or an approved sponsor.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to check
///
/// # Panics
///
/// Panics with "registry: unauthorized" if the address is neither admin nor a sponsor.
pub fn require_sponsor(e: &Env, addr: &Address) {
    let admin = storage::read_admin(e);
    if addr == &admin {
        return;
    }
    if !storage::is_sponsor(e, addr) {
        panic!("registry: unauthorized");
    }
}
