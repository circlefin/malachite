use std::{collections::HashMap, fmt::Debug};

use libp2p::{identify, PeerId, Swarm};
use tracing::info;

use crate::config;
use crate::{Discovery, DiscoveryClient};

use super::kademlia::KademliaSelector;
use super::random::RandomSelector;

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub(crate) fn get_selector(
        is_enabled: bool,
        bootstrap_protocol: config::BootstrapProtocol,
        selector: config::Selector,
    ) -> Box<dyn Selector<C>> {
        if !is_enabled {
            return Box::new(RandomSelector::new());
        }

        match selector {
            config::Selector::Kademlia => {
                if bootstrap_protocol != config::BootstrapProtocol::Kademlia {
                    panic!(
                        "Kademlia selector is only available with the Kademlia bootstrap protocol"
                    );
                }

                info!("Using Kademlia selector");
                Box::new(KademliaSelector::new())
            }

            config::Selector::Random => {
                info!("Using Random selector");
                Box::new(RandomSelector::new())
            }
        }
    }

    /// Excluded peers are those that are already outbound or have already
    /// been requested to be so.
    pub(crate) fn get_excluded_peers(&self) -> Vec<PeerId> {
        self.discovered_peers
            .keys()
            .filter(|peer_id| {
                self.outbound_peers.contains_key(peer_id)
                    || self.controller.connect_request.is_done_on(peer_id)
            })
            .cloned()
            .collect()
    }
}

pub enum Selection<T> {
    Exactly(Vec<T>),
    Only(Vec<T>),
    None,
}

impl<T> Selection<T> {
    /// Classifies a candidate list against the requested count `n`.
    ///
    /// Returns [`Selection::None`] when `n` is zero or no candidates are available,
    /// [`Selection::Only`] when fewer than `n` are available, and
    /// [`Selection::Exactly`] with exactly `n` peers when at least `n` were
    /// selected (surplus is truncated). Never returns an empty
    /// [`Selection::Exactly`].
    pub(crate) fn classify(mut candidates: Vec<T>, n: usize) -> Self {
        if n == 0 {
            return Self::None;
        }

        match candidates.len() {
            0 => Self::None,
            len if len < n => Self::Only(candidates),
            _ => {
                candidates.truncate(n);
                Self::Exactly(candidates)
            }
        }
    }
}

pub trait Selector<C>: Debug + Send
where
    C: DiscoveryClient,
{
    /// Try to select `n` valid outbound candidates. It might return less than `n`
    ///  candidates if there are not enough valid peers.
    fn try_select_n_outbound_candidates(
        &mut self,
        swarm: &mut Swarm<C>,
        discovered: &HashMap<PeerId, identify::Info>,
        excluded: Vec<PeerId>,
        n: usize,
    ) -> Selection<PeerId>;
}

#[cfg(test)]
mod tests {
    use super::Selection;

    #[test]
    fn classify_maps_empty_to_none() {
        assert!(matches!(
            Selection::<u8>::classify(vec![], 1),
            Selection::None
        ));
    }

    #[test]
    fn classify_maps_shortfall_to_only() {
        match Selection::classify(vec![1u8, 2], 3) {
            Selection::Only(peers) => assert_eq!(peers, vec![1, 2]),
            _ => panic!("expected Selection::Only"),
        }
    }

    #[test]
    fn classify_maps_full_count_to_exactly() {
        match Selection::classify(vec![1u8, 2], 2) {
            Selection::Exactly(peers) => assert_eq!(peers, vec![1, 2]),
            _ => panic!("expected Selection::Exactly"),
        }
    }

    #[test]
    fn classify_maps_zero_requested_to_none() {
        assert!(matches!(Selection::classify(vec![1u8], 0), Selection::None));
    }

    #[test]
    fn classify_truncates_surplus_to_requested_count() {
        match Selection::classify(vec![1u8, 2, 3], 2) {
            Selection::Exactly(peers) => assert_eq!(peers, vec![1, 2]),
            _ => panic!("expected Selection::Exactly"),
        }
    }
}
