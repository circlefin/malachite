use rand::rngs::StdRng;
use rand::SeedableRng;

use malachitebft_core_types::VotingPower;

use crate::{PrivateKey, Validator};

pub fn make_validators_seeded<const N: usize>(
    voting_powers: [VotingPower; N],
    seed: u64,
) -> [(Validator, PrivateKey); N] {
    let mut rng = StdRng::seed_from_u64(seed);

    let mut validators = Vec::with_capacity(N);

    for vp in voting_powers {
        let sk = PrivateKey::generate(&mut rng);
        let val = Validator::new(sk.public_key(), vp);
        validators.push((val, sk));
    }

    validators.try_into().expect("N validators")
}

pub fn make_validators<const N: usize>(
    voting_powers: [VotingPower; N],
) -> [(Validator, PrivateKey); N] {
    make_validators_seeded(voting_powers, 42)
}
