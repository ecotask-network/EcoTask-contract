use soroban_sdk::{contracttype, symbol_short, Address, Env, String};

/// Persistent storage TTL: re-bump on every touch; 4,096 ledgers (~5.7 days)
/// provides ample headroom since entries are refreshed on every interaction.
pub const PERSISTENT_TTL_THRESHOLD: u32 = 100;
pub const PERSISTENT_TTL_EXTEND_TO: u32 = 4_096;

/// Instance storage TTL: must survive the longest realistic quiet period
/// (holiday, upstream outage). 535,680 ledgers = 31 days at ~5 s/ledger.
pub const INSTANCE_TTL_THRESHOLD: u32 = 100;
pub const INSTANCE_TTL_EXTEND_TO: u32 = 535_680;

/// Extends the TTL of the contract instance (and code) to
/// `INSTANCE_TTL_EXTEND_TO` ledgers when it is within
/// `INSTANCE_TTL_THRESHOLD` ledgers of expiring.
///
/// Called as the first operation of every public entry point so that any
/// interaction with the token keeps its configuration alive.
pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Allowance {
    pub amount: i128,
    pub expiration_ledger: u32,
}

/// Writes the token balance for an address to storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to set the balance for
/// * `amount` - The balance amount to store
pub fn write_balance(e: &Env, addr: &Address, amount: i128) {
    let key = (symbol_short!("balance"), addr.clone());
    e.storage().persistent().set(&key, &amount);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}

/// Reads the token balance for an address from storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to query the balance for
///
/// # Returns
///
/// The balance as an i128, or 0 if no balance has been set.
pub fn read_balance(e: &Env, addr: &Address) -> i128 {
    let key = (symbol_short!("balance"), addr.clone());
    e.storage().persistent().get(&key).unwrap_or(0)
}

/// Writes the admin address to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `admin` - The admin address to store
pub fn write_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&symbol_short!("admin"), admin);
}

/// Reads the admin address from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored admin address.
///
/// # Panics
///
/// Panics if no admin has been set.
pub fn read_admin(e: &Env) -> Address {
    e.storage().instance().get(&symbol_short!("admin")).unwrap()
}

/// Checks if an admin address has been set.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// true if an admin exists, false otherwise.
pub fn has_admin(e: &Env) -> bool {
    e.storage().instance().has(&symbol_short!("admin"))
}

pub fn write_pending_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&symbol_short!("pending"), admin);
}

pub fn read_pending_admin(e: &Env) -> Option<Address> {
    e.storage().instance().get(&symbol_short!("pending"))
}

pub fn remove_pending_admin(e: &Env) {
    e.storage().instance().remove(&symbol_short!("pending"));
}

/// Writes the minter address to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `minter` - The minter address to store
pub fn write_minter(e: &Env, minter: &Address) {
    e.storage().instance().set(&symbol_short!("minter"), minter);
}

/// Reads the minter address from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored minter address.
///
/// # Panics
///
/// Panics if no minter has been set.
pub fn read_minter(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&symbol_short!("minter"))
        .unwrap()
}

/// Writes token metadata (name, symbol, decimal) to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `name` - The token name
/// * `symbol` - The token symbol
/// * `decimal` - The decimal precision
pub fn write_metadata(e: &Env, name: &String, symbol: &String, decimal: &u32) {
    e.storage().instance().set(&symbol_short!("name"), name);
    e.storage().instance().set(&symbol_short!("symbol"), symbol);
    e.storage()
        .instance()
        .set(&symbol_short!("decimal"), decimal);
}

/// Reads the token name from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored token name.
///
/// # Panics
///
/// Panics if no name has been set.
pub fn read_name(e: &Env) -> String {
    e.storage().instance().get(&symbol_short!("name")).unwrap()
}

/// Reads the token symbol from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored token symbol.
///
/// # Panics
///
/// Panics if no symbol has been set.
pub fn read_symbol(e: &Env) -> String {
    e.storage()
        .instance()
        .get(&symbol_short!("symbol"))
        .unwrap()
}

/// Reads the token decimal precision from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored decimal precision.
///
/// # Panics
///
/// Panics if no decimal has been set.
pub fn read_decimal(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&symbol_short!("decimal"))
        .unwrap()
}

/// Writes the total token supply to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `amount` - The total supply to store
pub fn write_supply(e: &Env, amount: i128) {
    e.storage()
        .instance()
        .set(&symbol_short!("supply"), &amount);
}

/// Reads the total token supply from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The total supply, or 0 if no supply has been set.
pub fn read_supply(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&symbol_short!("supply"))
        .unwrap_or(0)
}

/// Writes the maximum supply cap to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `amount` - The maximum supply cap to store
pub fn write_max_supply(e: &Env, amount: i128) {
    e.storage()
        .instance()
        .set(&symbol_short!("maxsupply"), &amount);
}

/// Reads the maximum supply cap from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The maximum supply cap if set, or None if no cap exists.
pub fn read_max_supply(e: &Env) -> Option<i128> {
    e.storage().instance().get(&symbol_short!("maxsupply"))
}

/// Writes an allowance for a spender to spend on behalf of an owner.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `owner` - The address that owns the tokens
/// * `spender` - The address authorized to spend
/// * `allowance` - The allowance details (amount and expiration)
pub fn write_allowance(e: &Env, owner: &Address, spender: &Address, allowance: &Allowance) {
    let key = (symbol_short!("allow"), owner.clone(), spender.clone());
    e.storage().persistent().set(&key, allowance);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}

/// Reads an allowance for a spender from storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `owner` - The address that granted the allowance
/// * `spender` - The address that was granted the allowance
///
/// # Returns
///
/// The allowance if it exists, or None if no allowance has been set.
pub fn read_allowance(e: &Env, owner: &Address, spender: &Address) -> Option<Allowance> {
    let key = (symbol_short!("allow"), owner.clone(), spender.clone());
    e.storage().persistent().get(&key)
}

/// Removes an allowance from persistent storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `owner` - The address that granted the allowance
/// * `spender` - The address that was granted the allowance
pub fn remove_allowance(e: &Env, owner: &Address, spender: &Address) {
    let key = (symbol_short!("allow"), owner.clone(), spender.clone());
    e.storage().persistent().remove(&key);
}

/// Spends (reduces) an allowance by a specified amount.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `owner` - The address that owns the tokens
/// * `spender` - The address authorized to spend
/// * `allowance` - The existing allowance details
/// * `amount` - The amount to deduct from the allowance
///
/// # Panics
///
/// Panics if the allowance would underflow.
pub fn spend_allowance(
    e: &Env,
    owner: &Address,
    spender: &Address,
    allowance: &Allowance,
    amount: i128,
) {
    let key = (symbol_short!("allow"), owner.clone(), spender.clone());
    let updated = Allowance {
        amount: allowance
            .amount
            .checked_sub(amount)
            .expect("allowance underflow"),
        expiration_ledger: allowance.expiration_ledger,
    };
    e.storage().persistent().set(&key, &updated);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}
