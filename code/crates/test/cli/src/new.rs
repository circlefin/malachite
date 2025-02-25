//! key and configuration generation

use rand::prelude::StdRng;
use rand::rngs::OsRng;
use rand::{Rng, SeedableRng};

use malachitebft_app::node::{CanGeneratePrivateKey, CanMakeGenesis, Node};
use malachitebft_core_types::{PrivateKey, PublicKey};

const MIN_VOTING_POWER: u64 = 1;
const MAX_VOTING_POWER: u64 = 1;

/// Generate private keys. Random or deterministic for different use-cases.
pub fn generate_private_keys<N>(
    node: &N,
    size: usize,
    deterministic: bool,
) -> Vec<PrivateKey<N::Context>>
where
    N: Node + CanGeneratePrivateKey,
{
    if deterministic {
        let mut rng = StdRng::seed_from_u64(0x42);
        (0..size)
            .map(|_| node.generate_private_key(&mut rng))
            .collect()
    } else {
        (0..size)
            .map(|_| node.generate_private_key(OsRng))
            .collect()
    }
}

/// Generate a Genesis file from the public keys and voting power.
/// Voting power can be random or deterministically pseudo-random.
pub fn generate_genesis<N>(
    node: &N,
    pks: Vec<PublicKey<N::Context>>,
    deterministic: bool,
) -> N::Genesis
where
    N: Node + CanMakeGenesis,
{
    let validators: Vec<_> = if deterministic {
        let mut rng = StdRng::seed_from_u64(0x42);
        pks.into_iter()
            .map(|pk| (pk, rng.gen_range(MIN_VOTING_POWER..=MAX_VOTING_POWER)))
            .collect()
    } else {
        pks.into_iter()
            .map(|pk| (pk, OsRng.gen_range(MIN_VOTING_POWER..=MAX_VOTING_POWER)))
            .collect()
    };

    node.make_genesis(validators)
}
