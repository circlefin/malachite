use std::collections::HashMap;

use libp2p::{identify, PeerId, Swarm};
use rand::seq::SliceRandom;
use tracing::{debug, warn};

use crate::DiscoveryClient;

use super::selector::{Selection, Selector};

#[derive(Debug)]
pub struct KademliaSelector {}

impl KademliaSelector {
    pub fn new() -> Self {
        KademliaSelector {}
    }

    fn kbuckets(&self, swarm: &mut Swarm<impl DiscoveryClient>) -> Vec<(u32, Vec<PeerId>)> {
        let mut kbuckets: Vec<(u32, Vec<PeerId>)> = Vec::new();

        for kbucket in swarm.behaviour_mut().kbuckets() {
            let peers = kbucket
                .iter()
                .map(|entry| *entry.node.key.preimage())
                .collect();
            let index = kbucket.range().0.ilog2().unwrap_or(0);
            kbuckets.push((index, peers));
        }

        kbuckets
    }
}

impl<C> Selector<C> for KademliaSelector
where
    C: DiscoveryClient,
{
    fn try_select_n_outbound_candidates(
        &mut self,
        swarm: &mut Swarm<C>,
        discovered: &HashMap<PeerId, identify::Info>,
        excluded: Vec<PeerId>,
        n: usize,
    ) -> Selection<PeerId> {
        if n == 0 {
            return Selection::None;
        }

        let mut candidates: Vec<PeerId> = Vec::new();

        let kbuckets_candidates: Vec<(u32, Vec<PeerId>)> = self
            .kbuckets(swarm)
            .into_iter()
            .map(|(index, peers)| {
                let filtered_peers = peers
                    .into_iter()
                    .filter(|peer_id| !excluded.contains(peer_id))
                    .collect();
                (index, filtered_peers)
            })
            .collect();

        if n < kbuckets_candidates.len() {
            warn!(
                "More kbuckets ({}) than the requested selection size ({})",
                kbuckets_candidates.len(),
                n
            );
        }

        let total_kbuckets_candidates: usize = kbuckets_candidates
            .iter()
            .map(|(_, peers)| peers.len())
            .sum();

        if total_kbuckets_candidates < n {
            for (_, peers) in &kbuckets_candidates {
                candidates.extend(peers.iter());
            }
        } else {
            // Select candidates in round-robin fashion based on kbucket index in reverse order
            for (_, peers) in kbuckets_candidates.iter().rev().cycle() {
                if candidates.len() >= n {
                    break;
                }
                if let Some(peer_id) = peers.iter().find(|peer_id| !candidates.contains(peer_id)) {
                    candidates.push(*peer_id);
                }
            }

            return Selection::Exactly(candidates);
        }

        debug!("Not enough peers in kbuckets, completing with random discovered peers");

        let mut rng = rand::thread_rng();
        let remaining = n - candidates.len();

        if discovered.len() < remaining {
            candidates.extend(discovered.keys().cloned());

            if candidates.is_empty() {
                return Selection::None;
            }
            return Selection::Only(candidates);
        }

        candidates.extend(
            discovered
                .keys()
                .filter(|peer_id| !candidates.contains(peer_id))
                .filter(|peer_id| !excluded.contains(peer_id))
                .cloned()
                .collect::<Vec<PeerId>>()
                .choose_multiple(&mut rng, remaining),
        );

        Selection::Exactly(candidates)
    }
}
