use crate::ValidatorSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genesis {
    pub validator_set: ValidatorSet,
}
