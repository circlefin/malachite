use std::collections::HashMap;

use libp2p::{identify, PeerId, Swarm};
use rand::seq::SliceRandom;

use crate::DiscoveryClient;

use super::selector::{Selection, Selector};

#[derive(Debug)]
pub struct RandomSelector {}

impl RandomSelector {
    pub fn new() -> Self {
        RandomSelector {}
    }
}

impl<C> Selector<C> for RandomSelector
where
    C: DiscoveryClient,
{
    fn try_select_n_outbound_candidates(
        &mut self,
        _swarm: &mut Swarm<C>,
        discovered: &HashMap<PeerId, identify::Info>,
        excluded: Vec<PeerId>,
        n: usize,
    ) -> Selection<PeerId> {
        if n == 0 {
            return Selection::None;
        }

        let mut discovered_candidates: Vec<PeerId> = discovered
            .keys()
            .filter(|peer_id| !excluded.contains(peer_id))
            .cloned()
            .collect();

        let mut rng = rand::thread_rng();
        discovered_candidates.shuffle(&mut rng);

        let candidates: Vec<PeerId> = discovered_candidates.into_iter().take(n).collect();

        match candidates.len() {
            0 => Selection::None,
            len if len < n => Selection::Only(candidates),
            _ => Selection::Exactly(candidates),
        }
    }
}
