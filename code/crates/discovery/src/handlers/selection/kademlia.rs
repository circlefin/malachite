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

/// Completes a partial k-bucket selection with random discovered peers that are
/// not already chosen and not excluded, then classifies the result via
/// [`Selection::classify`].
pub(crate) fn complete_with_discovered_peers(
    mut candidates: Vec<PeerId>,
    discovered: &HashMap<PeerId, identify::Info>,
    excluded: &[PeerId],
    n: usize,
) -> Selection<PeerId> {
    let remaining = n.saturating_sub(candidates.len());
    if remaining > 0 {
        let mut rng = rand::thread_rng();
        candidates.extend(
            discovered
                .keys()
                .filter(|peer_id| !candidates.contains(peer_id))
                .filter(|peer_id| !excluded.contains(peer_id))
                .cloned()
                .collect::<Vec<PeerId>>()
                .choose_multiple(&mut rng, remaining),
        );
    }

    Selection::classify(candidates, n)
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

        if total_kbuckets_candidates >= n {
            // Select candidates in round-robin fashion based on kbucket index in reverse order
            for (_, peers) in kbuckets_candidates.iter().rev().cycle() {
                if candidates.len() >= n {
                    break;
                }
                if let Some(peer_id) = peers.iter().find(|peer_id| !candidates.contains(peer_id)) {
                    candidates.push(*peer_id);
                }
            }

            return Selection::classify(candidates, n);
        }

        for (_, peers) in &kbuckets_candidates {
            candidates.extend(peers.iter());
        }

        debug!("Not enough peers in kbuckets, completing with random discovered peers");

        complete_with_discovered_peers(candidates, discovered, &excluded, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identify_info() -> identify::Info {
        identify::Info {
            public_key: libp2p::identity::Keypair::generate_ed25519().public(),
            protocol_version: String::new(),
            agent_version: String::new(),
            listen_addrs: vec![],
            protocols: vec![],
            observed_addr: libp2p::Multiaddr::empty(),
            signed_peer_record: None,
        }
    }

    fn discovered(peers: &[PeerId]) -> HashMap<PeerId, identify::Info> {
        peers
            .iter()
            .map(|peer_id| (*peer_id, identify_info()))
            .collect()
    }

    #[test]
    fn returns_none_when_all_discovered_peers_are_excluded() {
        let peer = PeerId::random();
        let selection = complete_with_discovered_peers(vec![], &discovered(&[peer]), &[peer], 1);

        assert!(matches!(selection, Selection::None));
    }

    #[test]
    fn returns_exactly_when_one_non_excluded_discovered_peer() {
        let peer = PeerId::random();
        let selection = complete_with_discovered_peers(vec![], &discovered(&[peer]), &[], 1);

        match selection {
            Selection::Exactly(peers) => assert_eq!(peers, vec![peer]),
            _ => panic!("expected Selection::Exactly"),
        }
    }

    #[test]
    fn returns_only_when_fewer_than_requested_after_exclusion() {
        let available = PeerId::random();
        let excluded = PeerId::random();
        let selection = complete_with_discovered_peers(
            vec![],
            &discovered(&[available, excluded]),
            &[excluded],
            2,
        );

        match selection {
            Selection::Only(peers) => {
                assert_eq!(peers, vec![available]);
                assert!(!peers.contains(&excluded));
            }
            _ => panic!("expected Selection::Only"),
        }
    }

    #[test]
    fn returns_none_when_discovered_and_kbucket_candidates_are_empty() {
        let selection = complete_with_discovered_peers(vec![], &HashMap::new(), &[], 1);

        assert!(matches!(selection, Selection::None));
    }

    #[test]
    fn truncates_to_n_when_candidates_already_exceed_request() {
        let peers: Vec<PeerId> = (0..3).map(|_| PeerId::random()).collect();
        let selection = complete_with_discovered_peers(peers.clone(), &HashMap::new(), &[], 1);

        match selection {
            Selection::Exactly(selected) => {
                assert_eq!(selected.len(), 1);
                assert_eq!(selected[0], peers[0]);
            }
            _ => panic!("expected Selection::Exactly"),
        }
    }
}
