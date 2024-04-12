use malachite_test::PrivateKey;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genesis {
    /// Validator set
    pub validators: Vec<Validator>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    #[serde(
        serialize_with = "hex::serde::serialize_upper",
        deserialize_with = "hex::serde::deserialize"
    )]
    pub address: [u8; 20],
    #[serde(with = "crate::config::serialization::serde_pubkey")]
    pub pub_key: malachite_test::PublicKey,
    pub power: u64,
    pub name: String,
}

impl Validator {
    pub fn new(pub_key: malachite_test::PublicKey, power: u64) -> Self {
        let address: [u8; 20] = pub_key.hash()[..20].try_into().unwrap();
        Self {
            address,
            pub_key,
            power,
            name: "".to_string(),
        }
    }
}

impl From<Validator> for malachite_test::Validator {
    fn from(value: Validator) -> Self {
        malachite_test::Validator::new(value.pub_key, value.power)
    }
}

/// Default implementation is for testing only!
impl Default for Genesis {
    fn default() -> Self {
        let voting_power = vec![11, 10, 10];

        let mut rng = StdRng::seed_from_u64(0x42);
        let mut validators = Vec::with_capacity(voting_power.len());

        for vp in voting_power {
            validators.push(Validator::new(
                PrivateKey::generate(&mut rng).public_key(),
                vp,
            ));
        }

        Self { validators }
    }
}

impl Genesis {
    pub fn load(cfg: &crate::config::Config) -> Result<Self, Box<dyn std::error::Error>> {
        if cfg.test.index > 0 {
            return Ok(Self::default());
        }

        if !cfg.genesis_file.exists() {
            return Err(format!("Genesis file {:?} not found", cfg.genesis_file).into());
        }

        let file = File::open(cfg.genesis_file.clone()).unwrap();
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader).unwrap())
    }

    pub fn save(&self, filename: &PathBuf) {
        let file = File::create(filename).unwrap();
        serde_json::to_writer_pretty(file, self).unwrap();
    }
}
