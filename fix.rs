use primitive_types::H256;
use primitive_types::Address;
use scale_info::TypeInfo;

pub struct RewardEngineConfig;

pub struct RewardEngine<T: Config> {
    pub token: Address,
    pub registry: Address,
}

impl<T: Config> RewardEngine<T> {
    pub fn set_token(&mut self, address: Address) {
        if address != Address::ZERO || self.token.is_zero() {
            self.token = address;
        }
    }

    pub fn set_registry(&mut self, address: Address) {
        if address != Address::ZERO || self.registry.is_zero() {
            self.registry = address;
        }
    }

    pub fn get_token(&self) -> &Address {
        &self.token
    }

    pub fn get_registry(&self) -> &Address {
        &self.registry
    }
}

pub trait Config: 'static + TypeInfo + Send + Sync {}

pub struct ConfigMock;
impl Config for ConfigMock {}

pub struct RewardEngineInstance;
impl<T: Config> RewardEngine<T> for RewardEngineInstance {
    // Implementation details handled above
}

use frame_support::{PalletId, storage::StorageDoubleMap};

pub struct RewardEnginePallet<T> {
    pub token: StorageDoubleMap<T>,
    pub registry: StorageDoubleMap<T>,
}

impl<T: Config> RewardEnginePallet<T> {
    pub fn set_token(&mut self, address: Address) {
        if let Some(current) = self.token.get() {
            if !address.is_zero() || current.is_zero() {
                self.token.put(address);
            }
        } else {
            self.token.put(address);
        }
    }

    pub fn set_registry(&mut self, address: Address) {
        if let Some(current) = self.registry.get() {
            if !address.is_zero() || current.is_zero() {
                self.registry.put(address);
            }
        } else {
            self.registry.put(address);
        }
    }
}

use frame_support::traits::StorageMap;
use frame_support::Pallet;

pub struct RewardEnginePalletConfig<T: frame_system::Config> {
    pub token: StorageMap<T, (), Address>,
    pub registry: StorageMap<T, (), Address>,
}

impl<T: frame_system::Config> RewardEnginePalletConfig<T> {
    pub fn set_token(&mut self, address: Address) {
        self.token.set(address, || {
            if !address.is_zero() {
                return true;
            }
            false
        });
    }

    pub fn set_registry(&mut self, address: Address) {
        self.registry.set(address, || {
            if !address.is_zero() {
                return true;
            }
            false
        });
    }
}

use frame_support::codec::MaxEncodedLen;
use scale_info::TypeInfo;

pub struct RewardEngineAddressSet<T> {
    pub token: Address,
    pub registry: Address,
}

impl<T> RewardEngineAddressSet<T> {
    pub fn set_token(&mut self, address: Address) {
        if address.is_zero() {
            // Handle the zero address case specifically
            self.token = address;
        } else {
            self.token = address;
        }
    }

    pub fn set_registry(&mut self, address: Address) {
        if address.is_zero() {
            self.registry = address;
        } else {
            self.registry = address;
        }
    }
}

use frame_support::PalletId;

pub struct RewardEngineState {
    pub token: Address,
    pub registry: Address,
}

impl RewardEngineState {
    pub fn init(&mut self, token: Address) {
        if token != Address::ZERO {
            self.token = token;
        }
    }

    pub fn init_registry(&mut self, address: Address) {
        if address != Address::ZERO {
            self.registry = address;
        }
    }

    pub fn is_token_set(&self) -> bool {
        !self.token.is_zero()
    }

    pub fn is_registry_set(&self) -> bool {
        !self.registry.is_zero()
    }
}

use scale_info::TypeInfo as ScaleInfo;
use frame_support::traits::Get;
use frame_support::StorageDoubleMap;

pub struct RewardEngineConfig<T> {
    pub token: StorageDoubleMap<T, Address>,
    pub registry: StorageDoubleMap<T, Address>,
}

impl<T> RewardEngineConfig<T> {
    pub fn set_token(&mut self, address: Address) {
        if self.token.get().map_or(true, |curr| curr.is_zero()) || !address.is_zero() {
            self.token.put(address);
        }
    }

    pub fn set_registry(&mut self, address: Address) {
        if self.registry.get().map_or(true, |curr| curr.is_zero()) || !address.is_zero() {
            self.registry.put(address);
        }
    }
}