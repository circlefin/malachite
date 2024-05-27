/// Example configurations for testing with indexed validators
use malachite_node::config::Config;
use malachite_test::ValidatorSet as Genesis;
use malachite_test::{PrivateKey, Validator};
use rand::prelude::StdRng;
use rand::SeedableRng;

/// Generate example configuration
pub fn generate_config(index: usize) -> Config {
    Config {
        moniker: format!("test-{}", index),
        ..Default::default()
    }
}

/// Generate an example genesis configuration
pub fn generate_genesis() -> Genesis {
    let voting_power = vec![11, 10, 10];

    let mut rng = StdRng::seed_from_u64(0x42);
    let mut validators = Vec::with_capacity(voting_power.len());

    for vp in voting_power {
        validators.push(Validator::new(
            PrivateKey::generate(&mut rng).public_key(),
            vp,
        ));
    }

    Genesis { validators }
}

/// Generate an example private key
pub fn generate_private_key(index: usize) -> PrivateKey {
    let mut rng = StdRng::seed_from_u64(0x42);
    for _ in 0..index {
        let _ = PrivateKey::generate(&mut rng);
    }
    PrivateKey::generate(&mut rng)
}
