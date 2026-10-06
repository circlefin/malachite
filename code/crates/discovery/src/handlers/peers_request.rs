use std::collections::HashSet;

use libp2p::{
    core::{PeerRecord, SignedEnvelope},
    request_response::{OutboundRequestId, ResponseChannel},
    PeerId, Swarm,
};
use tracing::{debug, error, trace, warn};

use crate::{
    behaviour::{self, Response, SignedPeerRecordBytes},
    dial::DialData,
    request::RequestData,
    Discovery, DiscoveryClient,
};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub fn can_peers_request(&self) -> bool {
        self.controller.peers_request.can_perform()
    }

    fn should_peers_request(&self, request_data: &RequestData) -> bool {
        // Has not already requested, or has requested but retries are allowed
        !self
            .controller
            .peers_request
            .is_done_on(&request_data.peer_id())
            || request_data.retry.count() != 0
    }

    /// Check rate limit for an incoming peers request.
    ///
    /// Returns `true` if the request should be served, `false` if rate limited.
    /// When rate limited, logs a warning and disconnects the peer if they've
    /// exceeded the maximum violation count.
    fn check_rate_limit(&mut self, swarm: &mut Swarm<C>, peer: &PeerId) -> bool {
        let result = self.rate_limiter.check_request(peer);

        if result.is_allowed() {
            return true;
        }

        let violation_count = self.rate_limiter.violation_count(peer);
        let should_disconnect = result.should_disconnect();

        warn!(
            %peer,
            violation_count,
            should_disconnect,
            "Rate limiting: ignoring peers request (exceeded {} requests in {:?})",
            self.rate_limiter.max_requests_per_window(),
            self.rate_limiter.rate_window()
        );

        // TODO: When reputation system is implemented:
        // - Report ReputationPenalty::ExcessivePeersRequests (-4096) to reputation system
        // - Reputation system will handle backoff tiers and eventual banning
        // - Maybe keep the immediate disconnect for max violations as fast path

        if should_disconnect {
            warn!(
                %peer,
                violation_count,
                max_violations = self.rate_limiter.max_violations(),
                "Disconnecting peer due to excessive peers request violations"
            );
            let _ = swarm.disconnect_peer_id(*peer);
        }

        false
    }

    pub fn peers_request_peer(&mut self, swarm: &mut Swarm<C>, request_data: RequestData) {
        if !self.is_enabled() || !self.should_peers_request(&request_data) {
            return;
        }

        self.controller
            .peers_request
            .register_done_on(request_data.peer_id());

        // Do not count retries as new interactions
        if request_data.retry.count() == 0 {
            self.metrics.increment_total_peer_requests();
        }

        debug!(
            "Requesting peers from peer {}, retry #{}",
            request_data.peer_id(),
            request_data.retry.count()
        );

        let request_id = swarm.behaviour_mut().send_request(
            &request_data.peer_id(),
            behaviour::Request::Peers(
                self.get_signed_peer_records_as_bytes(request_data.peer_id()),
            ),
        );

        self.controller
            .peers_request
            .register_in_progress(request_id, request_data);
    }

    pub(crate) fn handle_peers_request(
        &mut self,
        swarm: &mut Swarm<C>,
        peer: PeerId,
        channel: ResponseChannel<Response>,
        signed_records: Vec<SignedPeerRecordBytes>,
    ) {
        // Check rate limit and update violation tracking, may disconnect the peer.
        // Note: If discovery is disabled, this handler is never called (protocol not registered).
        if !self.check_rate_limit(swarm, &peer) {
            self.send_peers_response(swarm, peer, channel, Vec::new());
            return;
        }

        // Cap this verify pass at `max_peers_per_response`, matching
        // `process_signed_peer_records`. Extra records are not decoded, so a
        // peer with more known records than the cap may get a few
        // already-known records sent back to it.
        let received_peer_ids =
            peer_ids_from_signed_records(&signed_records, self.config.max_peers_per_response);

        // Process incoming signed records
        self.process_signed_peer_records(swarm, signed_records);

        // Send back only records they don't already have (the difference),
        // capped to max_peers_per_response to limit response size.
        let response_records: Vec<SignedPeerRecordBytes> = self
            .signed_peer_records
            .iter()
            .filter(|(pid, _)| **pid != peer && !received_peer_ids.contains(pid))
            .take(self.config.max_peers_per_response)
            .map(|(_, env)| env.clone().into_protobuf_encoding())
            .collect();

        self.send_peers_response(swarm, peer, channel, response_records);
    }

    /// Send a peers response with the given records.
    fn send_peers_response(
        &self,
        swarm: &mut Swarm<C>,
        peer: PeerId,
        channel: ResponseChannel<Response>,
        records: Vec<SignedPeerRecordBytes>,
    ) {
        let count = records.len();
        if swarm
            .behaviour_mut()
            .send_response(channel, behaviour::Response::Peers(records))
            .is_err()
        {
            error!(%peer, "Error sending peers response");
        } else {
            trace!(%peer, count, "Sent peers response");
        }
    }

    pub(crate) fn handle_peers_response(
        &mut self,
        swarm: &mut Swarm<C>,
        request_id: OutboundRequestId,
        signed_records: Vec<SignedPeerRecordBytes>,
    ) {
        self.controller
            .peers_request
            .remove_in_progress(&request_id);

        // Process signed records (verified peer_id, secure)
        self.process_signed_peer_records(swarm, signed_records);

        self.make_extension_step(swarm);
    }

    pub(crate) fn handle_failed_peers_request(
        &mut self,
        swarm: &mut Swarm<C>,
        request_id: OutboundRequestId,
    ) {
        if let Some(mut request_data) = self
            .controller
            .peers_request
            .remove_in_progress(&request_id)
        {
            if request_data.retry.count() < self.config.request_max_retries {
                // Retry request after a delay
                request_data.retry.inc_count();

                self.controller
                    .peers_request
                    .add_to_queue(request_data.clone(), Some(request_data.retry.next_delay()));
            } else {
                // No more trials left
                error!(
                    "Failed to send peers request to {0} after {1} trials",
                    request_data.peer_id(),
                    request_data.retry.count(),
                );

                self.metrics.increment_total_failed_peer_requests();

                self.make_extension_step(swarm);
            }
        }
    }

    /// Process received signed peer records
    /// Decode from bytes, verify signatures, dial valid peers
    fn process_signed_peer_records(
        &mut self,
        swarm: &mut Swarm<C>,
        signed_record_bytes: Vec<SignedPeerRecordBytes>,
    ) {
        let total = signed_record_bytes.len();
        let cap = self.config.max_peers_per_response;

        if total > cap {
            warn!(
                total,
                cap, "Received more peer records than allowed per response, truncating"
            );
        }

        for bytes in signed_record_bytes.into_iter().take(cap) {
            // Decode protobuf bytes to SignedEnvelope
            let envelope = match SignedEnvelope::from_protobuf_encoding(&bytes) {
                Ok(env) => env,
                Err(e) => {
                    warn!("Failed to decode signed envelope: {e}");
                    continue;
                }
            };

            // Verify and extract peer record
            match PeerRecord::from_signed_envelope(envelope) {
                Ok(peer_record) => {
                    let peer_id = peer_record.peer_id();
                    let addresses = peer_record.addresses().to_vec();

                    if addresses.is_empty() {
                        continue;
                    }

                    debug!(
                        %peer_id,
                        addr_count = addresses.len(),
                        "Received verified signed peer record"
                    );

                    // Dial gating (including persistent_peers_only) lives in should_dial.
                    self.add_to_dial_queue(swarm, DialData::new(Some(peer_id), addresses));
                }
                Err(e) => {
                    warn!("Invalid signed peer record: {e}");
                }
            }
        }
    }

    /// Get signed peer records as protobuf bytes, except for the given peer.
    /// Capped to `max_peers_per_response` to limit response size.
    fn get_signed_peer_records_as_bytes(&self, peer: PeerId) -> Vec<SignedPeerRecordBytes> {
        self.signed_peer_records
            .iter()
            .filter(|(peer_id, _)| **peer_id != peer)
            .take(self.config.max_peers_per_response)
            .map(|(_, envelope)| envelope.clone().into_protobuf_encoding())
            .collect()
    }
}

/// Decode and verify at most `cap` peer-supplied records.
/// Extra elements are ignored so this pass matches the sibling
/// `process_signed_peer_records` cap.
fn peer_ids_from_signed_records(
    signed_records: &[SignedPeerRecordBytes],
    cap: usize,
) -> HashSet<PeerId> {
    signed_records
        .iter()
        .take(cap)
        .filter_map(|bytes| {
            SignedEnvelope::from_protobuf_encoding(bytes)
                .ok()
                .and_then(|env| PeerRecord::from_signed_envelope(env).ok())
                .map(|rec| rec.peer_id())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use libp2p::identity::Keypair;
    use libp2p::Multiaddr;

    use super::*;

    fn signed_record_bytes() -> (PeerId, SignedPeerRecordBytes) {
        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from_public_key(&keypair.public());
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/1".parse().unwrap();
        let record = PeerRecord::new(&keypair, vec![addr]).expect("sign peer record");
        let bytes = record.into_signed_envelope().into_protobuf_encoding();
        (peer_id, bytes)
    }

    #[test]
    fn peer_ids_from_signed_records_stops_at_cap() {
        let (a, bytes_a) = signed_record_bytes();
        let (b, bytes_b) = signed_record_bytes();
        let (c, bytes_c) = signed_record_bytes();

        let ids = peer_ids_from_signed_records(&[bytes_a, bytes_b, bytes_c], 2);

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
        assert!(!ids.contains(&c));
    }

    #[test]
    fn peer_ids_from_signed_records_skips_malformed_before_verify() {
        let ids = peer_ids_from_signed_records(&[vec![0u8; 16], vec![1u8; 16]], 10);
        assert!(ids.is_empty());
    }
}
