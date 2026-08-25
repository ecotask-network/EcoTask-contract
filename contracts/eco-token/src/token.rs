use crate::storage;
use soroban_sdk::{contract, contractevent, contractimpl, Address, Env, String};

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintEvent {
    #[topic]
    pub admin: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferEvent {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnEvent {
    #[topic]
    pub from: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferFromEvent {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    #[topic]
    pub spender: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApproveEvent {
    #[topic]
    pub owner: Address,
    #[topic]
    pub spender: Address,
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaxSupplyUpdatedEvent {
    #[topic]
    pub admin: Address,
    pub max_supply: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataUpdatedEvent {
    #[topic]
    pub admin: Address,
    pub name: String,
    pub symbol: String,
    pub decimal: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinterUpdatedEvent {
    #[topic]
    pub admin: Address,
    pub previous_minter: Address,
    pub new_minter: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminProposedEvent {
    #[topic]
    pub current_admin: Address,
    #[topic]
    pub proposed_admin: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminAcceptedEvent {
    #[topic]
    pub new_admin: Address,
}

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    /// Initializes the token contract with admin, metadata, and initial minter.
    ///
    /// # Arguments
    ///
    /// * `admin` - The initial administrator address who can configure the contract
    /// * `name` - The human-readable name of the token (e.g., "ECO")
    /// * `symbol` - The token symbol/ticker (e.g., "ECO")
    /// * `decimal` - The number of decimal places for display purposes
    ///
    /// # Panics
    ///
    /// Panics if the contract has already been initialized.
    ///
    /// # Auth
    ///
    /// No authentication required. Can only be called once during deployment.
    pub fn initialize(e: Env, admin: Address, name: String, symbol: String, decimal: u32) {
        storage::extend_instance_ttl(&e);
        if storage::has_admin(&e) {
            panic!("token: already initialized");
        }
        storage::write_admin(&e, &admin);
        storage::write_minter(&e, &admin);
        storage::write_metadata(&e, &name, &symbol, &decimal);
        storage::write_supply(&e, 0);
    }

    /// Mints new tokens to a specified address.
    ///
    /// # Arguments
    ///
    /// * `to` - The address to receive the newly minted tokens
    /// * `amount` - The amount of tokens to mint (must be positive)
    ///
    /// # Panics
    ///
    /// * Panics if `amount <= 0`
    /// * Panics if minting would exceed the max supply cap (if set)
    ///
    /// # Auth
    ///
    /// Requires the caller to be the current minter address.
    pub fn mint(e: Env, to: Address, amount: i128) {
        storage::extend_instance_ttl(&e);
        let minter = storage::read_minter(&e);
        minter.require_auth();

        if amount <= 0 {
            panic!("token: amount must be positive");
        }

        let supply = storage::read_supply(&e);
        if let Some(cap) = storage::read_max_supply(&e) {
            let new_supply = supply.checked_add(amount).expect("supply overflow");
            if new_supply > cap {
                panic!("token: supply cap exceeded");
            }
        }

        let balance = storage::read_balance(&e, &to);
        storage::write_balance(
            &e,
            &to,
            balance.checked_add(amount).expect("balance overflow"),
        );

        storage::write_supply(&e, supply.checked_add(amount).expect("supply overflow"));

        MintEvent {
            admin: minter,
            to: to.clone(),
            amount,
        }
        .publish(&e);
    }

    /// Transfers tokens from one address to another.
    ///
    /// # Arguments
    ///
    /// * `from` - The address sending the tokens (must authorize the transfer)
    /// * `to` - The address receiving the tokens
    /// * `amount` - The amount of tokens to transfer (must be positive)
    ///
    /// # Panics
    ///
    /// * Panics if `amount <= 0`
    /// * Panics if `from` has insufficient balance
    ///
    /// # Auth
    ///
    /// Requires authentication from the `from` address.
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        storage::extend_instance_ttl(&e);
        from.require_auth();

        if amount <= 0 {
            panic!("token: amount must be positive");
        }

        let from_balance = storage::read_balance(&e, &from);
        if from_balance < amount {
            panic!("token: insufficient balance");
        }

        storage::write_balance(
            &e,
            &from,
            from_balance.checked_sub(amount).expect("balance underflow"),
        );

        let to_balance = storage::read_balance(&e, &to);
        storage::write_balance(
            &e,
            &to,
            to_balance.checked_add(amount).expect("balance overflow"),
        );

        TransferEvent {
            from: from.clone(),
            to: to.clone(),
            amount,
        }
        .publish(&e);
    }

    /// Returns the token balance of a given address.
    ///
    /// # Arguments
    ///
    /// * `id` - The address to query the balance for
    ///
    /// # Returns
    ///
    /// The current balance of the address as an i128.
    pub fn balance(e: Env, id: Address) -> i128 {
        storage::extend_instance_ttl(&e);
        storage::read_balance(&e, &id)
    }

    /// Returns the total supply of tokens currently in circulation.
    ///
    /// # Returns
    ///
    /// The total supply as an i128.
    pub fn total_supply(e: Env) -> i128 {
        storage::extend_instance_ttl(&e);
        storage::read_supply(&e)
    }

    /// The hard cap on total supply, or `i128::MAX` if no cap has been set.
    ///
    /// Minting is rejected whenever it would push the supply past this bound.
    ///
    /// # Returns
    ///
    /// The maximum supply cap, or i128::MAX if no cap is configured.
    pub fn max_supply(e: Env) -> i128 {
        storage::extend_instance_ttl(&e);
        storage::read_max_supply(&e).unwrap_or(i128::MAX)
    }

    /// Sets the hard supply cap. Admin-only. The cap must be positive and not
    /// lower than the current supply, so an already-oversubscribed token can
    /// never be retroactively frozen into an invalid state.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `max_supply` - The new maximum supply cap (must be positive and >= current supply)
    ///
    /// # Panics
    ///
    /// * Panics if caller is not the admin
    /// * Panics if `max_supply <= 0`
    /// * Panics if `max_supply < current_supply`
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_max_supply(e: Env, caller: Address, max_supply: i128) {
        storage::extend_instance_ttl(&e);
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("token: unauthorized");
        }
        if max_supply <= 0 {
            panic!("token: max supply must be positive");
        }
        if max_supply < storage::read_supply(&e) {
            panic!("token: max supply below current supply");
        }
        storage::write_max_supply(&e, max_supply);

        MaxSupplyUpdatedEvent { admin, max_supply }.publish(&e);
    }

    /// Returns the token name.
    ///
    /// # Returns
    ///
    /// The human-readable name of the token.
    pub fn name(e: Env) -> String {
        storage::extend_instance_ttl(&e);
        storage::read_name(&e)
    }

    /// Returns the token symbol/ticker.
    ///
    /// # Returns
    ///
    /// The token symbol (e.g., "ECO").
    pub fn symbol(e: Env) -> String {
        storage::extend_instance_ttl(&e);
        storage::read_symbol(&e)
    }

    /// Returns the number of decimal places for token display.
    ///
    /// # Returns
    ///
    /// The decimal precision as a u32.
    pub fn decimal(e: Env) -> u32 {
        storage::extend_instance_ttl(&e);
        storage::read_decimal(&e)
    }

    /// SEP-0041 alias for `decimal`.
    ///
    /// # Returns
    ///
    /// The number of decimal places for token display.
    pub fn decimals(e: Env) -> u32 {
        storage::extend_instance_ttl(&e);
        storage::read_decimal(&e)
    }

    /// Updates token metadata (SEP-0041 `set_metadata`). Admin-only.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `name` - The new token name
    /// * `symbol` - The new token symbol
    /// * `decimal` - The new decimal precision
    ///
    /// # Panics
    ///
    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_metadata(e: Env, caller: Address, name: String, symbol: String, decimal: u32) {
        storage::extend_instance_ttl(&e);
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("token: unauthorized");
        }
        storage::write_metadata(&e, &name, &symbol, &decimal);

        MetadataUpdatedEvent {
            admin,
            name,
            symbol,
            decimal,
        }
        .publish(&e);
    }

    /// Returns the current admin address.
    ///
    /// # Returns
    ///
    /// The address of the current administrator.
    pub fn admin(e: Env) -> Address {
        storage::extend_instance_ttl(&e);
        storage::read_admin(&e)
    }

    /// Transfers the admin role to a new address.
    ///
    /// # Arguments
    ///
    /// * `current_admin` - The current admin address (must authorize)
    /// * `new_admin` - The new admin address to transfer control to
    ///
    /// # Panics
    ///
    /// * Panics if `current_admin` is not the stored admin
    /// * Panics if `new_admin == current_admin`
    ///
    /// # Auth
    ///
    /// Requires authentication from the current admin address.
    pub fn propose_admin(e: Env, current_admin: Address, new_admin: Address) {
        storage::extend_instance_ttl(&e);
        current_admin.require_auth();
        let stored_admin = storage::read_admin(&e);
        if current_admin != stored_admin {
            panic!("token: unauthorized");
        }
        if new_admin == current_admin {
            panic!("token: new admin must be different");
        }
        storage::write_pending_admin(&e, &new_admin);
        AdminProposedEvent {
            current_admin,
            proposed_admin: new_admin,
        }
        .publish(&e);
    }

    pub fn accept_admin(e: Env, pending_admin: Address) {
        storage::extend_instance_ttl(&e);
        pending_admin.require_auth();
        let proposed =
            storage::read_pending_admin(&e).unwrap_or_else(|| panic!("token: no pending admin"));
        if pending_admin != proposed {
            panic!("token: unauthorized pending admin");
        }
        storage::write_admin(&e, &pending_admin);
        storage::remove_pending_admin(&e);
        AdminAcceptedEvent {
            new_admin: pending_admin,
        }
        .publish(&e);
    }

    // DEPRECATED: use propose_admin. This alias preserves the existing ABI while
    // requiring the proposed admin to accept before gaining control.
    pub fn transfer_admin(e: Env, current_admin: Address, new_admin: Address) {
        storage::extend_instance_ttl(&e);
        Self::propose_admin(e, current_admin, new_admin);
    }

    /// Returns the current minter address.
    ///
    /// # Returns
    ///
    /// The address authorized to mint new tokens.
    pub fn minter(e: Env) -> Address {
        storage::extend_instance_ttl(&e);
        storage::read_minter(&e)
    }

    /// Sets the minter address. Admin-only.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `new_minter` - The new address to authorize as minter
    ///
    /// # Panics
    ///
    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_minter(e: Env, caller: Address, new_minter: Address) {
        storage::extend_instance_ttl(&e);
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("token: unauthorized");
        }
        // The minting role must remain separate from the admin role. Allowing
        // `minter == admin` (the caller here) collapses the role separation the
        // security model depends on, so reject it explicitly.
        if new_minter == caller {
            panic!("token: minter must differ from admin");
        }
        let previous_minter = storage::read_minter(&e);
        storage::write_minter(&e, &new_minter);
        MinterUpdatedEvent {
            admin,
            previous_minter,
            new_minter,
        }
        .publish(&e);
    }

    /// Burns tokens from an address, reducing the total supply.
    ///
    /// # Arguments
    ///
    /// * `from` - The address to burn tokens from (must authorize)
    /// * `amount` - The amount of tokens to burn (must be positive)
    ///
    /// # Panics
    ///
    /// * Panics if `amount <= 0`
    /// * Panics if `from` has insufficient balance
    ///
    /// # Auth
    ///
    /// Requires authentication from the `from` address.
    pub fn burn(e: Env, from: Address, amount: i128) {
        storage::extend_instance_ttl(&e);
        from.require_auth();

        if amount <= 0 {
            panic!("token: amount must be positive");
        }

        let balance = storage::read_balance(&e, &from);
        if balance < amount {
            panic!("token: insufficient balance");
        }

        storage::write_balance(
            &e,
            &from,
            balance.checked_sub(amount).expect("balance underflow"),
        );

        let supply = storage::read_supply(&e);
        storage::write_supply(&e, supply.checked_sub(amount).expect("supply underflow"));

        BurnEvent {
            from: from.clone(),
            amount,
        }
        .publish(&e);
    }

    /// Approves a spender to transfer tokens on behalf of an owner.
    ///
    /// # Arguments
    ///
    /// * `owner` - The address owning the tokens (must authorize)
    /// * `spender` - The address being authorized to spend on behalf of owner
    /// * `amount` - The maximum amount the spender can transfer (must be non-negative)
    /// * `expiration_ledger` - The ledger sequence number at which this allowance expires
    ///
    /// # Panics
    ///
    /// * Panics if `amount < 0`
    /// * Panics if `expiration_ledger <= current_ledger_sequence`
    ///
    /// # Auth
    ///
    /// Requires authentication from the owner address.
    pub fn approve(e: Env, owner: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        storage::extend_instance_ttl(&e);
        owner.require_auth();

        if amount < 0 {
            panic!("token: amount must be non-negative");
        }
        if expiration_ledger <= e.ledger().sequence() {
            panic!("token: expiration must be in the future");
        }

        let allowance = storage::Allowance {
            amount,
            expiration_ledger,
        };
        storage::write_allowance(&e, &owner, &spender, &allowance);

        ApproveEvent {
            owner,
            spender,
            amount,
            expiration_ledger,
        }
        .publish(&e);
    }

    /// Returns the current allowance for a spender to transfer tokens from an owner.
    ///
    /// # Arguments
    ///
    /// * `owner` - The address that granted the allowance
    /// * `spender` - The address that was granted the allowance
    ///
    /// # Returns
    ///
    /// The current allowance amount, or 0 if no allowance exists or it has expired.
    pub fn allowance(e: Env, owner: Address, spender: Address) -> i128 {
        storage::extend_instance_ttl(&e);
        match storage::read_allowance(&e, &owner, &spender) {
            Some(a) => {
                if a.expiration_ledger < e.ledger().sequence() {
                    // Lazy cleanup: drop the expired allowance key from storage to
                    // reclaim ledger rent. The return value stays 0 per SEP-0041,
                    // so callers that only care about the amount are unaffected.
                    storage::remove_allowance(&e, &owner, &spender);
                    0
                } else {
                    a.amount
                }
            }
            None => 0,
        }
    }

    /// Returns an allowance together with its expiration ledger.
    ///
    /// # Arguments
    ///
    /// * `owner` - The address that granted the allowance
    /// * `spender` - The address that was granted the allowance
    ///
    /// # Returns
    ///
    /// Returns `None` when no allowance exists for the `(owner, spender)` pair.
    /// Otherwise, returns `Some((amount, expiration_ledger))`. The amount is zero
    /// when the allowance has expired, allowing callers to distinguish an expired
    /// approval from one that was never created.
    ///
    /// # Auth
    ///
    /// No authentication is required.
    pub fn allowance_with_expiry(e: Env, owner: Address, spender: Address) -> Option<(i128, u32)> {
        storage::extend_instance_ttl(&e);
        let current_sequence = e.ledger().sequence();
        match storage::read_allowance(&e, &owner, &spender) {
            Some(a) => {
                if a.expiration_ledger < current_sequence {
                    Some((0, a.expiration_ledger))
                } else {
                    Some((a.amount, a.expiration_ledger))
                }
            }
            None => None,
        }
    }

    /// Transfers tokens from one address to another using an approved allowance.
    ///
    /// # Arguments
    ///
    /// * `spender` - The address authorized to spend (must authorize)
    /// * `from` - The address owning the tokens
    /// * `to` - The address receiving the tokens
    /// * `amount` - The amount of tokens to transfer (must be positive)
    ///
    /// # Panics
    ///
    /// * Panics if `amount <= 0`
    /// * Panics if `from == to`
    /// * Panics if no allowance exists or it has expired
    /// * Panics if the allowance is insufficient for the transfer amount
    /// * Panics if `from` has insufficient balance
    ///
    /// # Auth
    ///
    /// Requires authentication from the spender address.
    pub fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        storage::extend_instance_ttl(&e);
        spender.require_auth();

        if amount <= 0 {
            panic!("token: amount must be positive");
        }

        if from == to {
            panic!("token: cannot transfer to self");
        }

        let allowance = match storage::read_allowance(&e, &from, &spender) {
            Some(a) => {
                if a.expiration_ledger < e.ledger().sequence() {
                    panic!("token: allowance expired");
                }
                a
            }
            None => panic!("token: allowance not found"),
        };

        if allowance.amount < amount {
            panic!("token: insufficient allowance");
        }

        storage::spend_allowance(&e, &from, &spender, &allowance, amount);

        let from_balance = storage::read_balance(&e, &from);
        if from_balance < amount {
            panic!("token: insufficient balance");
        }

        storage::write_balance(
            &e,
            &from,
            from_balance.checked_sub(amount).expect("balance underflow"),
        );

        let to_balance = storage::read_balance(&e, &to);
        storage::write_balance(
            &e,
            &to,
            to_balance.checked_add(amount).expect("balance overflow"),
        );

        TransferFromEvent {
            from,
            to,
            spender: spender.clone(),
            amount,
        }
        .publish(&e);
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use std::format;

    use crate::{TokenContract, TokenContractClient};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Ledger as _;
    use soroban_sdk::{Address, Env, String};

    #[test]
    fn test_initialize_and_metadata() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        assert_eq!(client.name(), String::from_str(&e, "ECO"));
        assert_eq!(client.symbol(), String::from_str(&e, "ECO"));
        assert_eq!(client.decimal(), 7);
        assert_eq!(client.total_supply(), 0);
    }

    #[test]
    fn test_mint_and_balance() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &1000);

        assert_eq!(client.balance(&user), 1000);
        assert_eq!(client.total_supply(), 1000);
    }

    #[test]
    fn test_transfer() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&from, &500);
        client.transfer(&from, &to, &300);

        assert_eq!(client.balance(&from), 200);
        assert_eq!(client.balance(&to), 300);
    }

    #[test]
    #[should_panic(expected = "token: amount must be positive")]
    fn test_mint_zero_amount() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &0);
    }

    #[test]
    #[should_panic(expected = "token: insufficient balance")]
    fn test_transfer_insufficient_balance() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.transfer(&from, &to, &100);
    }

    #[test]
    #[should_panic]
    fn test_mint_requires_minter_auth() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        client.mint(&user, &1000);
    }

    #[test]
    fn test_initial_minter_is_admin() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        assert_eq!(client.minter(), admin);
    }

    #[test]
    fn test_set_minter_transfers_minting_rights() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let minter = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_minter(&admin, &minter);
        assert_eq!(client.minter(), minter);

        client.mint(&user, &1000);
        assert_eq!(client.balance(&user), 1000);
        assert_eq!(client.total_supply(), 1000);
    }

    #[test]
    #[should_panic(expected = "token: unauthorized")]
    fn test_set_minter_unauthorized() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let attacker = Address::generate(&e);
        let minter = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_minter(&attacker, &minter);
    }

    #[test]
    fn test_set_minter_emits_minter_updated_event() {
        use soroban_sdk::testutils::Events as _;
        use soroban_sdk::vec;
        use soroban_sdk::{IntoVal, Symbol, Val};
        let e = Env::default();
        let admin = Address::generate(&e);
        let minter = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_minter(&admin, &minter);

        let events = e.events().all();
        let topics: soroban_sdk::Vec<Val> = vec![
            &e,
            Symbol::new(&e, "minter_updated_event").to_val(),
            admin.clone().into_val(&e),
        ];
        let data: Val = soroban_sdk::Map::<Symbol, Val>::from_array(
            &e,
            [
                (
                    Symbol::new(&e, "previous_minter"),
                    admin.clone().into_val(&e),
                ),
                (Symbol::new(&e, "new_minter"), minter.clone().into_val(&e)),
            ],
        )
        .into_val(&e);

        assert_eq!(events, vec![&e, (contract_id.clone(), topics, data)]);

        assert_eq!(client.minter(), minter);
    }

    #[test]
    #[should_panic(expected = "token: minter must differ from admin")]
    fn test_set_minter_rejects_admin_as_minter() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        // Setting the minter to the admin itself collapses role separation.
        client.set_minter(&admin, &admin);
    }

    #[test]
    #[should_panic(expected = "token: already initialized")]
    fn test_double_initialize_fails() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );
    }

    #[test]
    fn test_transfer_admin() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let new_admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.transfer_admin(&admin, &new_admin);
        assert_eq!(client.admin(), admin);
        client.accept_admin(&new_admin);
        assert_eq!(client.admin(), new_admin);
    }

    #[test]
    fn test_propose_admin_overwrite() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let first = Address::generate(&e);
        let second = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);
        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );
        e.mock_all_auths();
        client.propose_admin(&admin, &first);
        client.propose_admin(&admin, &second);
        client.accept_admin(&second);
        assert_eq!(client.admin(), second);
    }

    #[test]
    #[should_panic(expected = "token: unauthorized pending admin")]
    fn test_accept_admin_wrong_address() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let pending = Address::generate(&e);
        let wrong = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);
        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );
        e.mock_all_auths();
        client.propose_admin(&admin, &pending);
        client.accept_admin(&wrong);
    }

    #[test]
    #[should_panic(expected = "token: no pending admin")]
    fn test_accept_admin_without_proposal() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);
        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );
        e.mock_all_auths();
        client.accept_admin(&Address::generate(&e));
    }

    #[test]
    #[should_panic(expected = "token: unauthorized")]
    fn test_transfer_admin_unauthorized() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let attacker = Address::generate(&e);
        let new_admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.transfer_admin(&attacker, &new_admin);
    }

    #[test]
    #[should_panic(expected = "token: new admin must be different")]
    fn test_transfer_admin_same_address() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.transfer_admin(&admin, &admin);
    }

    #[test]
    fn test_burn() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &1000);
        client.burn(&user, &400);

        assert_eq!(client.balance(&user), 600);
        assert_eq!(client.total_supply(), 600);
    }

    #[test]
    #[should_panic(expected = "token: insufficient balance")]
    fn test_burn_more_than_balance() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &100);
        client.burn(&user, &200);
    }

    #[test]
    #[should_panic(expected = "token: amount must be positive")]
    fn test_burn_zero_amount() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.burn(&user, &0);
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let recipient = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&owner, &1000);
        client.approve(&owner, &spender, &500, &(e.ledger().sequence() + 100));

        assert_eq!(client.allowance(&owner, &spender), 500);

        client.transfer_from(&spender, &owner, &recipient, &300);

        assert_eq!(client.balance(&owner), 700);
        assert_eq!(client.balance(&recipient), 300);
        assert_eq!(client.allowance(&owner, &spender), 200);
    }

    #[test]
    fn test_transfer_from_emits_event() {
        use soroban_sdk::testutils::Events as _;
        use soroban_sdk::IntoVal;
        use soroban_sdk::{vec, Symbol, Val};

        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let recipient = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&owner, &1000);
        client.approve(&owner, &spender, &500, &(e.ledger().sequence() + 100));

        client.transfer_from(&spender, &owner, &recipient, &300);

        let events = e.events().all();
        let transfer_from_topics: soroban_sdk::Vec<Val> = vec![
            &e,
            Symbol::new(&e, "transfer_from_event").to_val(),
            owner.into_val(&e),
            recipient.into_val(&e),
            spender.into_val(&e),
        ];
        let transfer_from_data: Val = soroban_sdk::Map::<Symbol, Val>::from_array(
            &e,
            [(Symbol::new(&e, "amount"), 300i128.into_val(&e))],
        )
        .into_val(&e);

        assert_eq!(
            events,
            vec![&e, (contract_id, transfer_from_topics, transfer_from_data),]
        );
    }

    #[test]
    #[should_panic(expected = "token: insufficient allowance")]
    fn test_transfer_from_exceeds_allowance() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let recipient = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&owner, &1000);
        client.approve(&owner, &spender, &100, &(e.ledger().sequence() + 100));
        client.transfer_from(&spender, &owner, &recipient, &200);
    }

    #[test]
    #[should_panic(expected = "token: allowance not found")]
    fn test_transfer_from_no_allowance() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let recipient = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&owner, &1000);
        client.transfer_from(&spender, &owner, &recipient, &100);
    }

    #[test]
    #[should_panic(expected = "token: cannot transfer to self")]
    fn test_transfer_from_self_is_rejected() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&owner, &1000);
        client.approve(&owner, &spender, &500, &(e.ledger().sequence() + 100));
        client.transfer_from(&spender, &owner, &owner, &100);
    }

    #[test]
    fn test_zero_allowance_returns_zero() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        assert_eq!(client.allowance(&owner, &spender), 0);
    }

    #[test]
    fn test_allowance_with_expiry_live() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        let expiration_ledger = e.ledger().sequence() + 100;
        client.approve(&owner, &spender, &500, &expiration_ledger);

        // Live allowance: both allowance and allowance_with_expiry reflect the
        // live amount.
        assert_eq!(client.allowance(&owner, &spender), 500);
        assert_eq!(
            client.allowance_with_expiry(&owner, &spender),
            Some((500, expiration_ledger))
        );
    }

    #[test]
    fn test_allowance_with_expiry_expired() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        let expiration_ledger = e.ledger().sequence() + 50;
        client.approve(&owner, &spender, &500, &expiration_ledger);

        // Advance past the expiration ledger so the allowance is expired.
        e.ledger().set_sequence_number(expiration_ledger + 1);

        // allowance_with_expiry exposes the fact that an allowance WAS set and
        // is now expired (note: this must be read before allowance(), because
        // allowance() lazily cleans up the expired storage key).
        assert_eq!(
            client.allowance_with_expiry(&owner, &spender),
            Some((0, expiration_ledger))
        );
        // allowance() must still return 0.
        assert_eq!(client.allowance(&owner, &spender), 0);
    }

    #[test]
    fn test_allowance_with_expiry_none() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        // No allowance ever set: allowance is 0 and allowance_with_expiry is
        // None, distinguishing "never approved" from "expired".
        assert_eq!(client.allowance(&owner, &spender), 0);
        assert_eq!(client.allowance_with_expiry(&owner, &spender), None);
    }

    #[test]
    fn test_allowance_lazy_cleanup_of_expired() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        let expiration_ledger = e.ledger().sequence() + 50;
        client.approve(&owner, &spender, &500, &expiration_ledger);
        e.ledger().set_sequence_number(expiration_ledger + 1);

        // Before cleanup, allowance_with_expiry still sees the expired entry.
        assert_eq!(
            client.allowance_with_expiry(&owner, &spender),
            Some((0, expiration_ledger))
        );

        // Calling allowance() on the expired entry lazily removes the storage
        // key, so the expired allowance is now indistinguishable from "none".
        assert_eq!(client.allowance(&owner, &spender), 0);
        assert_eq!(client.allowance_with_expiry(&owner, &spender), None);
    }

    #[test]
    #[should_panic(expected = "token: amount must be positive")]
    fn test_mint_negative_amount() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &-100);
    }

    #[test]
    #[should_panic(expected = "token: amount must be positive")]
    fn test_transfer_negative_amount() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&from, &1000);
        client.transfer(&from, &to, &-50);
    }

    #[test]
    #[should_panic(expected = "token: amount must be positive")]
    fn test_burn_negative_amount() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.burn(&user, &-100);
    }

    #[test]
    fn test_transfer_to_self() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &1000);
        client.transfer(&user, &user, &500);

        assert_eq!(client.balance(&user), 1000);
    }

    #[test]
    fn test_approve_overwrites_previous() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.approve(&owner, &spender, &500, &(e.ledger().sequence() + 100));
        assert_eq!(client.allowance(&owner, &spender), 500);

        client.approve(&owner, &spender, &200, &(e.ledger().sequence() + 50));
        assert_eq!(client.allowance(&owner, &spender), 200);
    }

    #[test]
    fn test_burn_entire_balance() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &500);
        client.burn(&user, &500);

        assert_eq!(client.balance(&user), 0);
        assert_eq!(client.total_supply(), 0);
    }

    #[test]
    #[should_panic(expected = "token: amount must be non-negative")]
    fn test_approve_negative_amount() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.approve(&owner, &spender, &-100, &(e.ledger().sequence() + 100));
    }

    #[test]
    #[should_panic(expected = "token: expiration must be in the future")]
    fn test_approve_past_expiration() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.ledger().set_sequence_number(100);
        e.mock_all_auths();
        client.approve(&owner, &spender, &500, &50);
    }

    #[test]
    fn test_approve_zero_revokes_allowance() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.approve(&owner, &spender, &500, &(e.ledger().sequence() + 100));
        assert_eq!(client.allowance(&owner, &spender), 500);

        client.approve(&owner, &spender, &0, &(e.ledger().sequence() + 100));
        assert_eq!(client.allowance(&owner, &spender), 0);
    }

    #[test]
    fn test_max_supply_defaults_to_no_cap() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        assert_eq!(client.max_supply(), i128::MAX);
    }

    #[test]
    fn test_set_max_supply_and_mint_at_cap() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_max_supply(&admin, &1000);
        assert_eq!(client.max_supply(), 1000);

        client.mint(&user, &1000);
        assert_eq!(client.balance(&user), 1000);
        assert_eq!(client.total_supply(), 1000);
    }

    #[test]
    #[should_panic(expected = "token: supply cap exceeded")]
    fn test_mint_exceeding_supply_cap_fails() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_max_supply(&admin, &1000);
        client.mint(&user, &600);
        client.mint(&user, &500);
    }

    #[test]
    #[should_panic(expected = "token: supply cap exceeded")]
    fn test_mint_over_cap_on_first_issue_fails() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_max_supply(&admin, &500);
        client.mint(&user, &501);
    }

    #[test]
    #[should_panic(expected = "token: max supply must be positive")]
    fn test_set_zero_max_supply_fails() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_max_supply(&admin, &0);
    }

    #[test]
    #[should_panic(expected = "token: max supply below current supply")]
    fn test_set_max_supply_below_current_supply_fails() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&user, &1000);
        client.set_max_supply(&admin, &500);
    }

    #[test]
    #[should_panic(expected = "token: unauthorized")]
    fn test_set_max_supply_non_admin_fails() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let attacker = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_max_supply(&attacker, &1000);
    }

    #[test]
    fn test_set_metadata_updates_token_info() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_metadata(
            &admin,
            &String::from_str(&e, "EcoTask Token"),
            &String::from_str(&e, "ECOT"),
            &6,
        );

        assert_eq!(client.name(), String::from_str(&e, "EcoTask Token"));
        assert_eq!(client.symbol(), String::from_str(&e, "ECOT"));
        assert_eq!(client.decimal(), 6);
        assert_eq!(client.decimals(), 6);
    }

    #[test]
    #[should_panic(expected = "token: unauthorized")]
    fn test_set_metadata_non_admin_fails() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let attacker = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.set_metadata(
            &attacker,
            &String::from_str(&e, "Hijacked"),
            &String::from_str(&e, "HACK"),
            &7,
        );
    }

    #[test]
    fn test_decimals_matches_decimal() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        assert_eq!(client.decimals(), 7);
        assert_eq!(client.decimal(), client.decimals());
    }

    #[test]
    fn test_mint_at_i128_max_supply() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let user = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();

        // Verify default max_supply is i128::MAX
        assert_eq!(client.max_supply(), i128::MAX);

        // Mint i128::MAX amount (supply + amount == i128::MAX)
        client.mint(&user, &i128::MAX);
        assert_eq!(client.balance(&user), i128::MAX);
        assert_eq!(client.total_supply(), i128::MAX);

        // Minting 1 more token when supply + 1 overflows i128 should fail
        let res = client.try_mint(&user, &1);
        assert!(res.is_err());

        // Burn exact balance cleanly
        client.burn(&user, &i128::MAX);
        assert_eq!(client.balance(&user), 0);
        assert_eq!(client.total_supply(), 0);

        // Set max supply at exactly i128::MAX
        client.set_max_supply(&admin, &i128::MAX);
        assert_eq!(client.max_supply(), i128::MAX);

        // Minting up to i128::MAX succeeds with explicit cap set
        client.mint(&user, &i128::MAX);
        assert_eq!(client.balance(&user), i128::MAX);
        assert_eq!(client.total_supply(), i128::MAX);

        // Minting past cap panics via expect
        let res_cap = client.try_mint(&user, &1);
        assert!(res_cap.is_err());
    }

    #[test]
    fn test_transfer_to_balance_overflow_boundary() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();

        // Set up `to` balance at i128::MAX - 1 and `from` balance at 2 inside contract context
        e.as_contract(&contract_id, || {
            crate::storage::write_balance(&e, &to, i128::MAX - 1);
            crate::storage::write_supply(&e, i128::MAX - 1);
            crate::storage::write_balance(&e, &from, 2);
        });

        // Transfer 2 to `to` where `to` balance is i128::MAX - 1 and amount is 2.
        // Should overflow `to` balance and panic via expect("balance overflow").
        let res = client.try_transfer(&from, &to, &2);
        assert!(res.is_err());
    }

    use crate::storage::{INSTANCE_TTL_EXTEND_TO, PERSISTENT_TTL_EXTEND_TO};

    #[test]
    fn test_persistent_storage_survives_ledger_advancement() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let owner = Address::generate(&e);
        let spender = Address::generate(&e);
        let recipient = Address::generate(&e);
        let contract_id = e.register(TokenContract, ());
        let client = TokenContractClient::new(&e, &contract_id);

        client.initialize(
            &admin,
            &String::from_str(&e, "ECO"),
            &String::from_str(&e, "ECO"),
            &7,
        );

        e.mock_all_auths();
        client.mint(&owner, &1000);
        client.approve(&owner, &spender, &500, &(e.ledger().sequence() + 1_000_000));
        client.transfer(&owner, &recipient, &200);

        // Phase 1: Advance to the persistent TTL boundary (4,095 ledgers).
        // The balances and allowances were last bumped when written at ledger ~0,
        // so they must still be live at PERSISTENT_TTL_EXTEND_TO - 1.
        e.ledger().set_sequence_number(PERSISTENT_TTL_EXTEND_TO - 1);

        // Trigger any entry point to re-bump instance TTLs.
        let _ = client.total_supply();

        // Persistent entries (balance, allowance) must survive.
        assert_eq!(client.balance(&owner), 800);
        assert_eq!(client.balance(&recipient), 200);
        assert_eq!(client.allowance(&owner, &spender), 500);

        // Instance entries (admin, metadata, supply) must survive.
        assert_eq!(client.admin(), admin);
        assert_eq!(client.name(), String::from_str(&e, "ECO"));
        assert_eq!(client.total_supply(), 1000);

        // Phase 2: Advance past the persistent TTL. Persistent entries have
        // been evicted (their TTL was 4,096). Re-create them by calling
        // mutating entry points, then verify everything works.
        e.ledger().set_sequence_number(INSTANCE_TTL_EXTEND_TO - 1);

        // Re-touch balance via mint, re-touch allowance via approve.
        client.mint(&owner, &1);
        client.approve(&owner, &spender, &500, &(e.ledger().sequence() + 1_000_000));

        // All persistent entries re-bumped and alive.
        assert_eq!(client.balance(&owner), 801);
        assert_eq!(client.allowance(&owner, &spender), 500);

        // Instance entries still alive.
        assert_eq!(client.admin(), admin);
        assert_eq!(client.total_supply(), 1001);
    }

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn proptest_mint_below_cap(amount in 1..=i128::MAX) {
            let e = Env::default();
            let admin = Address::generate(&e);
            let user = Address::generate(&e);
            let contract_id = e.register(TokenContract, ());
            let client = TokenContractClient::new(&e, &contract_id);

            client.initialize(
                &admin,
                &String::from_str(&e, "ECO"),
                &String::from_str(&e, "ECO"),
                &7,
            );

            e.mock_all_auths();

            // mint with random amount in [1, i128::MAX] never panics when supply remains below cap
            client.mint(&user, &amount);
            prop_assert_eq!(client.balance(&user), amount);
            prop_assert_eq!(client.total_supply(), amount);
        }

        #[test]
        fn proptest_transfer_preserves_total_supply(
            mint_amount in 1..=i128::MAX,
            transfer_amount in 1..=i128::MAX,
        ) {
            let e = Env::default();
            let admin = Address::generate(&e);
            let from = Address::generate(&e);
            let to = Address::generate(&e);
            let contract_id = e.register(TokenContract, ());
            let client = TokenContractClient::new(&e, &contract_id);

            client.initialize(
                &admin,
                &String::from_str(&e, "ECO"),
                &String::from_str(&e, "ECO"),
                &7,
            );

            e.mock_all_auths();
            client.mint(&from, &mint_amount);

            if transfer_amount <= mint_amount {
                let res = client.try_transfer(&from, &to, &transfer_amount);
                prop_assert!(res.is_ok());
                prop_assert_eq!(client.balance(&from), mint_amount - transfer_amount);
                prop_assert_eq!(client.balance(&to), transfer_amount);
                // Total supply MUST be preserved
                prop_assert_eq!(client.total_supply(), mint_amount);
            } else {
                let res = client.try_transfer(&from, &to, &transfer_amount);
                prop_assert!(res.is_err());
                prop_assert_eq!(client.balance(&from), mint_amount);
                prop_assert_eq!(client.total_supply(), mint_amount);
            }
        }

        #[test]
        fn proptest_burn_preserves_total_supply(
            mint_amount in 1..=i128::MAX,
            burn_amount in 1..=i128::MAX,
        ) {
            let e = Env::default();
            let admin = Address::generate(&e);
            let user = Address::generate(&e);
            let contract_id = e.register(TokenContract, ());
            let client = TokenContractClient::new(&e, &contract_id);

            client.initialize(
                &admin,
                &String::from_str(&e, "ECO"),
                &String::from_str(&e, "ECO"),
                &7,
            );

            e.mock_all_auths();
            client.mint(&user, &mint_amount);

            if burn_amount <= mint_amount {
                let res = client.try_burn(&user, &burn_amount);
                prop_assert!(res.is_ok());
                prop_assert_eq!(client.balance(&user), mint_amount - burn_amount);
                prop_assert_eq!(client.total_supply(), mint_amount - burn_amount);
                if burn_amount == mint_amount {
                    prop_assert_eq!(client.balance(&user), 0);
                    prop_assert_eq!(client.total_supply(), 0);
                }
            } else {
                let res = client.try_burn(&user, &burn_amount);
                prop_assert!(res.is_err());
                prop_assert_eq!(client.balance(&user), mint_amount);
                prop_assert_eq!(client.total_supply(), mint_amount);
            }
        }
    }
}
