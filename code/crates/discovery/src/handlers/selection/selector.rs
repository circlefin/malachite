use libp2p::{identify, PeerId, Swarm};
use std::{collections::HashMap, fmt::Debug};
use tracing::info;

use crate::{Discovery, DiscoveryClient};

use super::{kademlia::KademliaSelector, random::RandomSelector};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub(crate) fn get_selector(name: &str) -> Box<dyn Selector<C>> {
        match name {
            "kademlia" => {
                info!("Using Kademlia selector");
                Box::new(KademliaSelector::new())
            }
            "random" => {
                info!("Using Random selector");
                Box::new(RandomSelector::new())
            }
            _ => panic!("Unknown selector: {}", name),
        }
    }

    /// Excluded peers are those that are already outbound connections or have already
    /// been requested to be so.
    pub(crate) fn get_excluded_peers(&self) -> Vec<PeerId> {
        self.discovered_peers
            .keys()
            .filter(|peer_id| {
                self.outbound_connections.contains_key(peer_id)
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
