// TODO: Abstract over all of this

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    pub fn hash(&self) -> u64 {
        // TODO
        self.0.iter().fold(0, |acc, x| acc ^ *x as u64)
    }
}

pub type VotingPower = u64;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address(u64);

impl Address {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

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
        Address(self.public_key.hash()) // TODO
    }
}

/// A validator set contains a list of validators sorted by address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorSet {
    validators: Vec<Validator>,
}

impl ValidatorSet {
    pub fn new(validators: impl IntoIterator<Item = Validator>) -> Self {
        let mut validators: Vec<_> = validators.into_iter().collect();
        ValidatorSet::sort_validators(&mut validators);

        Self { validators }
    }

    /// The total voting power of the validator set
    pub fn total_voting_power(&self) -> VotingPower {
        // TODO: Cache this?
        self.validators.iter().map(|v| v.voting_power).sum()
    }

    /// Add a validator to the set
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

    /// Remove a validator from the set
    pub fn remove(&mut self, val: Validator) {
        self.validators.retain(|v| v.address() != val.address());

        Self::sort_validators(&mut self.validators); // TODO: Not needed
    }

    /// Get a validator by its address
    pub fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.validators.iter().find(|v| &v.address() == address)
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
