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
