// TODO: Abstract over all of this

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)] // TODO: Remove PartialOrd, Ord
pub struct PublicKey(Vec<u8>);

pub type VotingPower = u64;

// TODO: Use an actual address
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address(PublicKey);

/// A validator is a public key and voting power
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validator {
    pub public_key: PublicKey,
    pub voting_power: VotingPower,
}

impl Validator {
    pub fn new(public_key: PublicKey, voting_power: VotingPower) -> Self {
        Self {
            public_key,
            voting_power,
        }
    }

    pub fn hash(&self) -> Vec<u8> {
        self.public_key.0.clone() // TODO
    }

    pub fn address(&self) -> Address {
        Address(self.public_key.clone()) // TODO
    }
}

/// A validator set contains a list of validators sorted by address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorSet {
    validators: Vec<Validator>,
    total_voting_power: VotingPower,
}

impl ValidatorSet {
    pub fn new(validators: impl IntoIterator<Item = Validator>) -> Self {
        let mut validators: Vec<_> = validators.into_iter().collect();
        ValidatorSet::sort_validators(&mut validators);

        let total_voting_power = validators.iter().map(|v| v.voting_power).sum();

        Self {
            validators,
            total_voting_power,
        }
    }

    pub fn total_voting_power(&self) -> VotingPower {
        self.total_voting_power
    }

    pub fn add(&mut self, validator: Validator) {
        self.validators.push(validator);
        ValidatorSet::sort_validators(&mut self.validators);
    }

    /// Update the voting power of the given validator
    pub fn update(&mut self, val: Validator) {
        if let Some(v) = self
            .validators
            .iter_mut()
            .find(|v| v.address() == val.address())
        {
            v.voting_power = val.voting_power;
        }

        Self::sort_validators(&mut self.validators);
    }

    pub fn remove(&mut self, val: Validator) {
        self.validators.retain(|v| v.address() != val.address());

        Self::sort_validators(&mut self.validators); // TODO: Not needed
    }

    /// In place sort and deduplication of a list of validators
    fn sort_validators(vals: &mut Vec<Validator>) {
        use core::cmp::Reverse;

        // Sort the validators according to the current Tendermint requirements
        // (v. 0.34 -> first by validator power, descending, then by address, ascending)
        vals.sort_unstable_by_key(|v| (Reverse(v.voting_power), v.address()));

        vals.dedup();
    }
}
