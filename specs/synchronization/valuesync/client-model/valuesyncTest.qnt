// -*- mode: quint -*-
//
// Tests for the ValueSync model. Run with:
//   quint test valuesyncTest.qnt
//
module valuesyncTest {
  import valuesync(
    PEERS = Set("p1", "p2", "p3"),
    BATCH_SIZE = 5,
    MAX_PARALLEL = 5,
    TOP = 25,
    HISTORY_MIN = 1
  ).* from "./valuesync"

  run zeroParallelRequestsDefaultToOneTest = all {
    effectiveParallelRequests(0) == 1,
    effectiveParallelRequests(5) == 5,
  }

  // --------------------------------------------------------------------------
  // Happy path: a lagging node requests a batch, gets a full response, and
  // decides both heights in order — catching up cleanly.
  // --------------------------------------------------------------------------
  run happyCatchUpTest =
    init
      .then(startConsensus)
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))          // p1 oracle tip = 2
      .then(receiveStatus("p1"))        // known p1 tip = 2
      .then(requestValues("p1"))        // send [1..2] to p1, sync_height -> 3
      .then(deliverFull(0))             // buffer heights 1,2; entry becomes a reservation
      .then(decide)                     // decide height 1
      .then(decide)                     // decide height 2
      .expect(client.tip == 2 and invNoLostHeight and invSyncUncovered and invDisjoint)

  // A prefix-only retry rolls syncHeight back so the remaining suffix stays requestable.
  run prefixRetryPreservesCoverageTest =
    init
      .then(startConsensus)             // started, tip=0, sync_height=1
      .then(peerAdvance("p2"))
      .then(peerAdvance("p2"))          // p2 oracle tip = 2 (whole-range peer)
      .then(peerAdvance("p3"))          // p3 oracle tip = 1 (prefix-only peer)
      .then(receiveStatus("p2"))
      .then(receiveStatus("p3"))
      .then(requestValues("p2"))        // send [1..2] to p2, sync_height -> 3
      // The response was invalid, so p2 is excluded and only p3 can serve the retry. That limits
      // the requested range to [1..1].
      .then(retryServed(0, "p3", true))
      .expect(client.tip == 0 and client.syncHeight == 2)
      .expect(coversHeight(client.pending, 1))
      .expect(invNoLostHeight)
      .expect(invSyncAboveTip and invSyncUncovered and invDisjoint)

  // Disconnecting a peer reroutes its pending range without losing suffix coverage.
  run disconnectRetryPreservesCoverageTest =
    init
      .then(startConsensus)
      .then(peerAdvance("p2"))
      .then(peerAdvance("p2"))          // p2 tip = 2
      .then(peerAdvance("p3"))          // p3 tip = 1
      .then(receiveStatus("p2"))
      .then(receiveStatus("p3"))
      .then(requestValues("p2"))        // send [1..2] to p2, sync_height -> 3
      .then(peerDisconnect("p2"))
      .expect(client.tip == 0 and client.syncHeight == 2)
      .expect(not(client.known.keys().contains("p2")))   // p2 gone
      .expect(coversHeight(client.pending, 1))            // prefix requested
      .expect(invNoLostHeight)                            // suffix remains requestable

  // A changed pruning floor can leave only a prefix-capable peer for the
  // remainder of a partial response without losing suffix coverage.
  run partialResponseAfterStatusChangePreservesCoverageTest =
    init
      .then(startConsensus)
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))          // p1 can initially serve [1..3]
      .then(peerAdvance("p2"))
      .then(peerAdvance("p2"))          // p2 can serve only through height 2
      .then(receiveStatus("p1"))
      .then(receiveStatus("p2"))
      .then(requestValues("p1"))        // send [1..3] to p1, sync_height -> 4
      .then(peerPrune("p1"))
      .then(peerPrune("p1"))            // p1 now retains [3..3]
      .then(receiveStatus("p1"))        // p1 no longer covers suffix start 2
      .then(deliverPartial(0, 1))           // keep [1..1], return [2..2] to the frontier
      .then(requestValues("p2"))            // the frontier pass requests [2..2]
      .expect(client.syncHeight == 3)
      .expect(coversHeight(client.pending, 1) and coversHeight(client.pending, 2))
      .expect(not(coversHeight(client.pending, 3)))
      .expect(invNoLostHeight)
      .expect(invSyncAboveTip and invSyncUncovered and invDisjoint and invNoOrphanedOwner)

  // A completed reservation must not consume the capacity needed to request the next uncovered
  // height from an eligible peer.
  run staleReservationDoesNotBlockFrontierTest =
    init
      .then(startConsensus)
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))             // p1 oracle tip = 25
      .then(receiveStatus("p1"))            // advertise tip 25
      .then(requestValues("p1"))            // [1..5]
      .then(requestValues("p1"))            // [6..10]
      .then(requestValues("p1"))            // [11..15]
      .then(requestValues("p1"))            // [16..20]
      .then(requestValues("p1"))            // [21..25], capacity reached
      .then(decideFromConsensus)             // decide height 1; no range is pruned
      .then(receiveAdvertisedStatus("p1", 1))
      .then(deliverPartial(0, 1))            // keep [1..1], leave height 2 uncovered
      .then(deliverFull(1))
      .then(deliverFull(2))
      .then(deliverFull(3))
      .then(deliverFull(4))
      .then(receiveStatus("p1"))            // p1 can serve [2..5] again
      .then(requestValues("p1"))            // request the uncovered [2..5] frontier
      .expect(client.tip == 1 and client.consensusHeight == 2 and client.syncHeight == 26)
      .expect(client.pending.keys().size() == MAX_PARALLEL)
      .expect(client.pending.keys().exists(id =>
        client.pending.get(id).range == { lo: 2, hi: 5 } and client.pending.get(id).inflight))
      .expect(invSyncAboveTip and invSyncUncovered and invDisjoint and invNoOrphanedOwner
        and invNoLostHeight and invNoCapacityBlockedFrontier)

  // A missing height below completed reservations stays requestable: reservations for higher
  // heights must not consume the capacity needed to re-request the one gap that keeps every
  // one of them from being decided and pruned.
  run missingLowerHeightStaysRequestableTest =
    init
      .then(startConsensus)
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))            // p1 oracle tip = 25
      .then(receiveStatus("p1"))
      .then(requestValues("p1"))          // [1..5]
      .then(requestValues("p1"))          // [6..10]
      .then(requestValues("p1"))          // [11..15]
      .then(requestValues("p1"))          // [16..20]
      .then(requestValues("p1"))          // [21..25], capacity reached
      // The [1..5] request times out. That excludes p1, the only peer that could serve the range,
      // so the entry is dropped and syncHeight rolls back to 1. One slot is now free and height 1
      // is uncovered.
      .then(retryUnserved(0, true))
      .expect(client.syncHeight == 1 and client.pending.keys().size() == 4)
      // The response to [6..10] covers only height 6. The prefix is kept as a reservation and
      // the remaining [7..10] returns to the frontier rather than taking the freed slot.
      .then(deliverPartial(1, 1))
      .then(deliverFull(2))
      .then(deliverFull(3))
      .then(deliverFull(4))
      // Every slot now holds a completed reservation for a height above the missing height 1.
      // None can be decided while height 1 is missing, and none can time out.
      .expect(client.tip == 0 and client.syncHeight == 1)
      .expect(invNoCapacityBlockedFrontier)
      .then(requestValues("p1"))          // re-request the missing [1..5]
      .expect(client.pending.keys().exists(id =>
        client.pending.get(id).range == { lo: 1, hi: 5 } and client.pending.get(id).inflight))

  // A reservation whose values turn out to be bad is re-requested through
  // `on_peer_fault` or `on_local_transient_error`. It holds no request slot, so that retry adds a
  // request the budget never accounted for. It must wait for a free slot instead.
  run reservationRetryStaysInsideBudgetTest =
    init
      .then(startConsensus)
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(peerAdvance("p1"))
      .then(receiveStatus("p1"))
      .then(requestValues("p1"))          // [1..5]
      .then(requestValues("p1"))          // [6..10]
      .then(requestValues("p1"))          // [11..15]
      .then(requestValues("p1"))          // [16..20]
      .then(requestValues("p1"))          // [21..25], budget full
      .expect(inflightCount == 5)
      // A one-height prefix arrives. The entry becomes a reservation and frees its slot.
      .then(deliverPartial(0, 1))
      .expect(inflightCount == 4 and not(client.pending.get(0).inflight))
      // The freed slot goes to the range just above the reservation, so the budget is full again.
      .then(requestValues("p1"))
      .expect(inflightCount == 5)
      // The reservation now has no slot to return to, so `retryServed` is not enabled.
      .expect(budgetAfterRemoval(0) >= maxParallelRequests)
      .then(retryUnserved(0, true))
      .expect(inflightCount == 5 and invBudgetRespected)
      // Its range is uncovered again, its values are gone, and the frontier points at it.
      .expect(client.syncHeight == 1 and not(client.buffered.contains(1)))
      .expect(invSyncAboveTip and invSyncUncovered and invDisjoint and invNoOrphanedOwner
        and invNoLostHeight and invNoCapacityBlockedFrontier)
}
