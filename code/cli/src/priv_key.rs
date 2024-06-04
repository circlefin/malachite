use serde::{Deserialize, Serialize};

use malachite_test::{Address, PrivateKey, PublicKey};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivValidatorKey {
    pub address: Address,
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
}

impl From<PrivateKey> for PrivValidatorKey {
    fn from(private_key: PrivateKey) -> Self {
        let public_key = private_key.public_key();
        let address = Address::from_public_key(&public_key);

        Self {
            address,
            public_key,
            private_key,
        }
    }
}
