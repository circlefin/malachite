// -*- mode: quint -*-
//
// AI-generated model of the Malachite ValueSync protocol (client side + peer oracle).
//
// Grounds:  code/crates/sync/src/handle.rs, code/crates/sync/src/state.rs
//           (and the engine driver code/crates/engine/src/sync.rs for buffering/decide)
//
// Scope:    a single syncing node running the full ValueSync CLIENT state machine,
//           against an abstract PEER ORACLE. Peers are ground-truth sources of decided
//           values; the client learns their tips via Status and requests height ranges.
//
// Abstractions (see the scope block at the bottom for full details):
//   - A height IS its value: commit certificates are assumed valid, so the value at a
//     height is canonical. A node's decided values form a contiguous prefix [1..tip].
//   - The EMA peer scorer is dropped: peer selection is nondeterministic among the peers
//     `filter_peers_by_range` would return (whole-range preferred, prefix fallback — see
//     `peerEligible`); only the score-weighting within that eligible set is abstracted away
//     (scoring is a performance heuristic, not a safety mechanism).
//   - libp2p transport / codecs / metrics are out.
//   - All request-failure triggers are UNIFIED (identical effect on sync state: exclude
//     the peer and re-request via `re_request_values_from_peer_except`): timeout, invalid
//     response, network/dial failure (SyncRequestFailed), and peer disconnect
//     (PeerDisconnected). Modeled by the `retry*` actions.
module valuesync {
  // ----------------------------------------------------------------------------
  // Constants (system-model parameters)
  // ----------------------------------------------------------------------------
  const PEERS: Set[str]        // the set of peers the client can sync from
  const BATCH_SIZE: int        // config.batch_size — max heights per request
  const MAX_PARALLEL: int      // config.parallel_requests — max concurrent pending requests
  const TOP: int               // max height available anywhere in the network
  const HISTORY_MIN: int       // initial earliest height retained by each peer

  pure val INITIAL: int = 1    // first block height; height 0 (ZERO) means "no block yet"

  // Constant upper bound for height iteration. Apalache requires the bounds of `a.to(b)` to be
  // constants, so height scans iterate `0.to(MAXH)` with a filter rather than a dynamic range.
  // Every height, sync_height (<= TOP+1), and the "next uncovered" answer stays below MAXH, and
  // MAXH itself is always uncovered (pending ranges never exceed TOP), so it is a safe sentinel.
  pure val MAXH: int = TOP + BATCH_SIZE + 2

  // ----------------------------------------------------------------------------
  // Types
  // ----------------------------------------------------------------------------
  type Height = int
  type PeerId = str
  type ReqId  = int

  // Inclusive height range; empty iff hi < lo.
  type Range = { lo: Height, hi: Height }

  // A peer's advertised capability: it can serve [historyMin .. tip].
  type Status = { tip: Height, historyMin: Height }

  // One entry of the client's pending_requests map.
  //   inflight = true  : a network request is outstanding (can get a response / time out)
  //   inflight = false : a placeholder RESERVATION (prefix kept after a partial response,
  //                       or a full response awaiting consensus) — pruned when tip passes it
  type PendingEntry = {
    range: Range,
    peer: PeerId,
    excluded: Set[PeerId],   // peers already tried & failed for this range (accumulated)
    inflight: bool,
  }

  // The full client-side sync state (sync::State), plus the engine's buffered-values set.
  type Client = {
    started: bool,
    consensusHeight: Height,          // height consensus is voting on (not yet decided)
    tip: Height,                      // last decided height
    syncHeight: Height,               // next height to request; INV: syncHeight > tip
    pending: ReqId -> PendingEntry,
    known: PeerId -> Status,          // last Status heard per peer (may lag tip/pruning changes)
    buffered: Set[Height],            // valid synced values held, awaiting consensus decide
    nextReqId: ReqId,                 // monotonic id allocator (opaque)
    // Latched once a retry fires on a reservation. Which entry a retry started from is a property
    // of the step, not of any state it leads to, so a witness over states cannot see it.
    retriedReservation: bool,
  }

  // ----------------------------------------------------------------------------
  // State variables
  // ----------------------------------------------------------------------------
  var client: Client
  var oracle: PeerId -> Status        // ground truth: what each peer can actually serve

  // ----------------------------------------------------------------------------
  // Pure logic — range helpers
  // ----------------------------------------------------------------------------
  pure def imin(a: int, b: int): int = if (a < b) a else b
  pure def imax(a: int, b: int): int = if (a > b) a else b
  pure def rEmpty(r: Range): bool = r.hi < r.lo
  pure def rLen(r: Range): int = if (r.hi < r.lo) 0 else r.hi - r.lo + 1
  pure def rContains(r: Range, h: Height): bool = r.lo <= h and h <= r.hi
  pure def rDisjoint(a: Range, b: Range): bool = a.hi < b.lo or b.hi < a.lo
  pure def suffixOf(r: Range, k: int): Range = { lo: r.lo + k, hi: r.hi }

  // ----------------------------------------------------------------------------
  // Pure logic — pending-request helpers (state.rs / handle.rs)
  // ----------------------------------------------------------------------------
  pure def coversHeight(pending: ReqId -> PendingEntry, h: Height): bool =
    pending.keys().exists(id => rContains(pending.get(id).range, h))

  pure def removeReq(pending: ReqId -> PendingEntry, rid: ReqId): ReqId -> PendingEntry =
    pending.keys().exclude(Set(rid)).mapBy(id => pending.get(id))

  pure def removePeer(known: PeerId -> Status, p: PeerId): PeerId -> Status =
    known.keys().exclude(Set(p)).mapBy(q => known.get(q))

  // handle.rs `find_next_uncovered_height`: smallest h >= start covered by no pending range.
  // Bounded search: at most TOP heights >= start can be covered, so start+TOP+1 is always free.
  pure def firstUncoveredFrom(pending: ReqId -> PendingEntry, start: Height): Height =
    // Smallest h >= start not covered by any pending range. MAXH is >= start and always
    // uncovered, so it is in the filtered set and bounds the fold from above.
    0.to(MAXH).filter(h => h >= start and not(coversHeight(pending, h))).fold(MAXH, (m, h) => imin(m, h))

  // handle.rs `set_sync_height`: floor to tip+1, then skip to the next uncovered height.
  pure def setSyncHeightP(tip: Height, pending: ReqId -> PendingEntry, candidate: Height): Height =
    firstUncoveredFrom(pending, imax(tip + 1, candidate))

  // handle.rs `find_next_uncovered_range_from`: a batch starting at the first uncovered
  // height >= initial, capped at BATCH_SIZE and at the next pending range's boundary.
  pure def minLo(rs: Set[Range]): int =
    rs.fold(TOP + 2, (m, r) => imin(m, r.lo))

  pure def findNextUncoveredRangeFrom(pending: ReqId -> PendingEntry, initial: Height, maxSize: int): Range =
    val adjusted = firstUncoveredFrom(pending, initial)
    val maxEnd = adjusted + imax(1, maxSize) - 1
    val blockers = pending.keys().map(id => pending.get(id).range).filter(r => r.hi >= adjusted)
    val end = if (blockers == Set()) maxEnd else imin(maxEnd, minLo(blockers) - 1)
    { lo: adjusted, hi: end }

  // handle.rs `trim_validated_heights`: drop the prefix already validated by consensus.
  pure def trimValidated(r: Range, tip: Height): Range = { lo: imax(tip + 1, r.lo), hi: r.hi }

  // handle.rs `prune_pending_requests`: drop entries fully at/below the tip.
  pure def prunePending(pending: ReqId -> PendingEntry, tip: Height): ReqId -> PendingEntry =
    pending.keys().filter(id => pending.get(id).range.hi > tip).mapBy(id => pending.get(id))

  // ----------------------------------------------------------------------------
  // Pure logic — peer eligibility (state.rs `filter_peers_by_range`)
  // A peer can serve the WHOLE range, or at least a PREFIX starting at range.lo.
  // ----------------------------------------------------------------------------
  pure def canWhole(s: Status, r: Range): bool =
    s.historyMin <= r.lo and r.lo <= r.hi and r.hi <= s.tip
  pure def canPrefix(s: Status, r: Range): bool =
    s.historyMin <= r.lo and r.lo <= s.tip
  pure def servingRange(s: Status, r: Range): Range = { lo: r.lo, hi: imin(r.hi, s.tip) }

  // `filter_peers_by_range` PREFERS whole-range peers: a prefix peer is only eligible when no
  // whole-range peer exists (both excluding `except`). Modeling this preference — rather than
  // any prefix-capable peer — is what keeps peer selection faithful to the code.
  pure def wholePeerExists(known: PeerId -> Status, r: Range, except: Set[PeerId]): bool =
    known.keys().exists(q => not(q.in(except)) and canWhole(known.get(q), r))

  pure def peerEligible(known: PeerId -> Status, p: PeerId, r: Range, except: Set[PeerId]): bool =
    not(p.in(except)) and canPrefix(known.get(p), r)
      and (canWhole(known.get(p), r) or not(wholePeerExists(known, r, except)))

  // Peers that can serve at least a prefix of `r` (excluding `except`).
  pure def eligiblePeers(known: PeerId -> Status, r: Range, except: Set[PeerId]): Set[PeerId] =
    known.keys().filter(q => not(q.in(except)) and canPrefix(known.get(q), r))

  // Pick the highest-tip peer from a (non-empty) candidate set. This realizes the whole-range
  // preference of `filter_peers_by_range`: if any candidate can serve the whole range its tip is
  // highest, so it wins; otherwise the longest-prefix peer wins. "" is a never-a-peer seed,
  // replaced on the first fold step (candidates are non-empty at every call site).
  pure def maxTipPeer(known: PeerId -> Status, cands: Set[PeerId]): PeerId =
    cands.fold("", (best, q) =>
      if (best == "" or known.get(q).tip > known.get(best).tip) q else best)

  // ----------------------------------------------------------------------------
  // Pure logic — the "send a request" core (send_and_track_request_to_peer)
  // ----------------------------------------------------------------------------
  // Insert a fresh inflight entry for `fr` to `p`, then advance syncHeight past `fr`
  // ONLY if syncHeight sat inside fr (handle.rs:812 guard); always re-validate via setSyncHeight.
  pure def doSend(c: Client, p: PeerId, fr: Range, excluded: Set[PeerId]): Client =
    val rid = c.nextReqId
    val entry = { range: fr, peer: p, excluded: excluded, inflight: true }
    val pending1 = c.pending.put(rid, entry)
    val candidate = if (rContains(fr, c.syncHeight)) fr.hi + 1 else c.syncHeight
    { ...c, pending: pending1, nextReqId: rid + 1,
            syncHeight: setSyncHeightP(c.tip, pending1, candidate) }

  // Roll back syncHeight toward a range's start.
  pure def rollbackSync(c: Client, rangeStart: Height): Client =
    { ...c, syncHeight: setSyncHeightP(c.tip, c.pending, imin(c.syncHeight, rangeStart)) }

  // ----------------------------------------------------------------------------
  // Pure logic — response processing
  // ----------------------------------------------------------------------------
  // Buffer the k received heights [lo .. lo+k-1] that are still ahead of the tip.
  pure def addBuffered(c: Client, lo: Height, k: int): Set[Height] =
    c.buffered.union(0.to(MAXH).filter(h => lo <= h and h <= lo + k - 1 and h > c.tip))

  // Partial response: buffer the prefix, then retain its reservation only while it is above tip.
  pure def applyPartialShrink(c: Client, rid: ReqId, k: int): Client =
    val e = c.pending.get(rid)
    val prefix = { lo: e.range.lo, hi: e.range.lo + k - 1 }
    val pending1 = c.pending.set(rid, { ...e, range: prefix, inflight: false })
    { ...c, buffered: addBuffered(c, e.range.lo, k),
            pending: prunePending(pending1, c.tip) }

  // Full response: buffer everything, keep the entry as a reservation (inflight := false).
  pure def applyFull(c: Client, rid: ReqId): Client =
    val e = c.pending.get(rid)
    { ...c, buffered: addBuffered(c, e.range.lo, rLen(e.range)),
            pending: c.pending.set(rid, { ...e, inflight: false }) }

  // Send a range to `p`, rolling syncHeight back when `p` can serve only a prefix.
  pure def sendRangePreservingSuffix(c: Client, p: PeerId, reqRange: Range, excluded: Set[PeerId]): Client =
    val fr = servingRange(c.known.get(p), reqRange)
    val c2 = doSend(c, p, fr, excluded)
    if (fr.hi < reqRange.hi)
      { ...c2, syncHeight: setSyncHeightP(c2.tip, c2.pending, imin(c2.syncHeight, fr.hi + 1)) }
    else
      c2

  // What a retry does to the heights of the range that are already buffered. The two triggers
  // differ, so the caller says which one it is.
  //
  //   `faulted` = true  — `Msg::PeerFault` drops every buffered value that came from the same
  //                       response before it calls into sync, so the siblings go with it.
  //   `faulted` = false — `Msg::LocalTransientError` only arms a backoff timer. It never touches
  //                       the queue, so buffered siblings stay.
  //
  // Both triggers lose the value at `consensusHeight`. `buffered` stands for every value consensus
  // can still reach, and a fault means consensus took that one and rejected it. The code never
  // holds it: `process_value_response` sends a value at the current height straight on and buffers
  // only greater heights, and a drained value leaves the queue before consensus sees it.
  //
  // The rest matters only for a reservation. An in-flight entry has buffered nothing yet.
  pure def dropBuffered(c: Client, r: Range, faulted: bool): Set[Height] =
    val kept = if (faulted) c.buffered.exclude(0.to(MAXH).filter(h => rContains(r, h)))
               else c.buffered
    kept.exclude(Set(c.consensusHeight))

  // Who a retry bars from serving the range next. `re_request_values_from_peer_except` adds the
  // serving peer only when the caller names one. `on_local_transient_error` passes `None`, because
  // the failure is the node's own, so the same peer stays eligible.
  pure def retryExcluded(e: PendingEntry, faulted: bool): Set[PeerId] =
    if (faulted) e.excluded.union(Set(e.peer)) else e.excluded

  // re_request (invalid/timeout/failed): drop the entry, bar whoever the trigger blames, and
  // re-request the whole range to `np` (whose serving range may be only a prefix).
  pure def doRetry(c: Client, rid: ReqId, np: PeerId, faulted: bool): Client =
    val e = c.pending.get(rid)
    val c1 = { ...c, pending: removeReq(c.pending, rid),
                     buffered: dropBuffered(c, e.range, faulted) }
    sendRangePreservingSuffix(c1, np, e.range, retryExcluded(e, faulted))

  // re_request with no eligible peer left, or with a full budget: drop the entry, rollback
  // syncHeight to range.lo.
  pure def doRetryRollback(c: Client, rid: ReqId, faulted: bool): Client =
    val e = c.pending.get(rid)
    val c1 = { ...c, pending: removeReq(c.pending, rid),
                     buffered: dropBuffered(c, e.range, faulted) }
    { ...c1, syncHeight: setSyncHeightP(c1.tip, c1.pending, imin(c1.syncHeight, e.range.lo)) }

  // Reroute one in-flight entry `rid` (owned by now-disconnected `dp`) to the best eligible peer
  // (highest tip = whole-range preferred), or roll syncHeight back to its start if no peer can
  // serve even a prefix. This is the per-range body of `on_peer_disconnected`'s loop over
  // `re_request_values_from_peer_except` (which selects a peer per range via
  // `random_peer_with_except`), so it must NOT roll back merely because one particular peer is a
  // prefix peer — only when the eligible set is empty. A disconnect is not a fault and only
  // reaches in-flight entries, so nothing buffered is dropped here.
  pure def rerouteEntry(c: Client, rid: ReqId, dp: PeerId): Client =
    val e = c.pending.get(rid)
    val c1 = { ...c, pending: removeReq(c.pending, rid) }
    val except = e.excluded.union(Set(dp))
    val cands = eligiblePeers(c1.known, e.range, except)
    if (cands == Set())
      { ...c1, syncHeight: setSyncHeightP(c1.tip, c1.pending, imin(c1.syncHeight, e.range.lo)) }
    else
      sendRangePreservingSuffix(c1, maxTipPeer(c1.known, cands), e.range, except)

  // Decide the next height (fused Decided(nt) + StartedHeight(nt+1) for a lagging node).
  pure def applyDecide(c: Client): Client =
    val nt = c.tip + 1
    val pending1 = prunePending(c.pending, nt)
    { ...c, tip: nt, consensusHeight: nt + 1,
            buffered: c.buffered.exclude(Set(nt)),
            pending: pending1,
            syncHeight: setSyncHeightP(nt, pending1, imax(c.syncHeight, nt + 1)) }

  // ----------------------------------------------------------------------------
  // Derived quantities
  // ----------------------------------------------------------------------------
  // Only requests still awaiting a response consume the parallel-request budget. A reservation
  // reserves its range for coverage, but it holds no request.
  val inflightCount: int =
    client.pending.keys().filter(id => client.pending.get(id).inflight).size()

  // state.rs `max_parallel_requests`: zero configured slots default to one.
  pure def effectiveParallelRequests(configured: int): int = imax(1, configured)
  pure val maxParallelRequests: int = effectiveParallelRequests(MAX_PARALLEL)
  pure val readAheadWindow: int = maxParallelRequests * imax(1, BATCH_SIZE)

  // state.rs `read_ahead_limit`: a new batch starts at most one full request budget above the tip.
  // A batch that starts at the limit may extend beyond it.
  val readAheadLimit: Height = client.tip + readAheadWindow

  // `re_request_values_from_peer_except` removes the entry before it checks the budget, so an
  // entry that was in flight gives its own slot back first. A reservation holds no slot, so
  // re-requesting one adds a request the budget has not accounted for.
  def budgetAfterRemoval(rid: ReqId): int =
    if (client.pending.get(rid).inflight) inflightCount - 1 else inflightCount

  // The next range `request_values` asks for, without the heights consensus already has.
  val frontierRange: Range =
    trimValidated(findNextUncoveredRangeFrom(client.pending, client.syncHeight, BATCH_SIZE),
                  client.tip)

  // The capacity guard of `request_values`: a slot is free and the frontier is inside the
  // read-ahead window. `requestValues` and `capacityBlockedFrontier` share this definition, so
  // the forbidden state stays tied to what the implementation can actually do.
  val frontierRequestable: bool = and {
    inflightCount < maxParallelRequests,
    not(rEmpty(frontierRange)),
    frontierRange.lo <= readAheadLimit,
  }

  // The frontier is starved when the node can neither decide nor request its way forward: no
  // request is outstanding, consensus lacks the next value, an eligible peer holds the gap, and
  // still no request can be sent.
  val capacityBlockedFrontier: bool = and {
    client.started,
    inflightCount == 0,
    not(client.buffered.contains(client.tip + 1)),
    not(rEmpty(frontierRange)),
    client.known.keys().exists(p => peerEligible(client.known, p, frontierRange, Set())),
    not(frontierRequestable),
  }

  // ============================================================================
  // Actions
  // ============================================================================
  action init = all {
    client' = {
      started: false, consensusHeight: 0, tip: 0, syncHeight: 0,
      pending: Map(),
      known: Map(),   // state.peers starts empty; peers appear as Status arrives
      buffered: Set(),
      nextReqId: 0,
      retriedReservation: false,
    },
    oracle' = PEERS.mapBy(_ => { tip: 0, historyMin: HISTORY_MIN }),
  }

  // Consensus starts at INITIAL for the first time (on_started_height, Start).
  action startConsensus = {
    val c1 = { ...client, started: true, consensusHeight: INITIAL, tip: INITIAL - 1 }
    all {
      not(client.started),
      client' = { ...c1, syncHeight: setSyncHeightP(c1.tip, c1.pending, INITIAL) },
      oracle' = oracle,
    }
  }

  // A peer decides its next height (chain grows). Ground-truth advance.
  action peerAdvance(p: PeerId): bool = all {
    oracle.get(p).tip < TOP,
    oracle' = oracle.set(p, { ...oracle.get(p), tip: oracle.get(p).tip + 1 }),
    client' = client,
  }

  // A peer prunes its oldest retained height. The client keeps using the previous
  // capability until a later Status publishes the new history minimum.
  action peerPrune(p: PeerId): bool = all {
    oracle.get(p).historyMin < oracle.get(p).tip,
    oracle' = oracle.set(p, { ...oracle.get(p), historyMin: oracle.get(p).historyMin + 1 }),
    client' = client,
  }

  // The client hears a peer's current status after its tip or pruning floor changes.
  // `put` adds a newly-heard peer or updates a known one — models `state.peers.insert`.
  action receiveStatus(p: PeerId): bool = all {
    client' = { ...client, known: client.known.put(p, oracle.get(p)) },
    oracle' = oracle,
  }

  // Status is peer-provided and may advertise a stale tip below the peer's actual tip.
  action receiveAdvertisedStatus(p: PeerId, advertisedTip: Height): bool = all {
    advertisedTip >= oracle.get(p).historyMin,
    advertisedTip <= oracle.get(p).tip,
    client' = { ...client, known: client.known.put(p,
      { ...oracle.get(p), tip: advertisedTip }) },
    oracle' = oracle,
  }

  // A peer disconnects (NetworkEvent::PeerDisconnected -> on_peer_disconnected): ATOMICALLY
  // remove it from the known-peer set AND reroute every in-flight range it owned via the same
  // retry primitive (`re_request_values_from_peer_except`), exactly as the handler does within
  // one input path. Doing both in one action avoids a model-only transient state where a
  // disconnected peer still owns pending work (which the real synchronous handler never
  // exposes to other inputs).
  //
  // Each owned range is rerouted independently to the best eligible peer (whole-range preferred,
  // matching `random_peer_with_except`/`filter_peers_by_range`), or rolled back only if no
  // eligible peer remains for that range — exactly as `on_peer_disconnected`'s per-range loop does.
  action peerDisconnect(p: PeerId): bool = {
    val cRemoved = { ...client, known: removePeer(client.known, p) }
    // Only the peer's IN-FLIGHT entries. A reservation already holds its values, so
    // on_peer_disconnected leaves it in place rather than spending a slot and buffering a second
    // copy. Such a reservation outlives its owner until the tip prunes it (see invNoOrphanedOwner).
    val pEntries = client.pending.keys().filter(id =>
      client.pending.get(id).peer == p and client.pending.get(id).inflight)
    all {
      client.known.keys().contains(p),
      client' = pEntries.fold(cRemoved, (c, rid) => rerouteEntry(c, rid, p)),
      oracle' = oracle,
    }
  }

  // Proactively request one uncovered batch from an eligible peer (request_values, one slot).
  action requestValues(p: PeerId): bool = all {
    client.started,
    frontierRequestable,
    client.known.keys().contains(p),
    peerEligible(client.known, p, frontierRange, Set()),
    client' = doSend(client, p, servingRange(client.known.get(p), frontierRange), Set()),
    oracle' = oracle,
  }

  // A full response arrives for an inflight request.
  action deliverFull(rid: ReqId): bool = all {
    rid != -1,
    client.pending.keys().contains(rid),
    client.pending.get(rid).inflight,
    client' = applyFull(client, rid),
    oracle' = oracle,
  }

  // A partial (prefix of length k) response arrives. The entry shrinks to the received prefix and
  // the suffix returns to the frontier; `requestValues` picks it up like any other gap, so there
  // is no separate suffix scheduler and no separate served/unserved case.
  action deliverPartial(rid: ReqId, k: int): bool = all {
    rid != -1,
    client.pending.keys().contains(rid),
    client.pending.get(rid).inflight,
    k >= 1,
    k < rLen(client.pending.get(rid).range),
    client' = rollbackSync(applyPartialShrink(client, rid, k),
                           client.pending.get(rid).range.lo + k),
    oracle' = oracle,
  }

  // Which entries a failure can reach. An in-flight entry fails through an invalid response or a
  // timeout, so any range qualifies. A reservation fails through `on_peer_fault` or
  // `on_local_transient_error`, which both fire while consensus processes a value, so only the
  // reservation holding `consensusHeight` qualifies. `get_request_id_by` then finds it by range
  // membership alone, without testing the in-flight flag.
  def retryable(rid: ReqId): bool =
    client.pending.get(rid).inflight
      or rContains(client.pending.get(rid).range, client.consensusHeight)

  // A tracked range fails and is re-requested to np.
  action retryServed(rid: ReqId, np: PeerId, faulted: bool): bool = all {
    rid != -1,
    client.pending.keys().contains(rid),
    retryable(rid),
    budgetAfterRemoval(rid) < maxParallelRequests,
    client.known.keys().contains(np),
    peerEligible(client.known, np, client.pending.get(rid).range,
                 retryExcluded(client.pending.get(rid), faulted)),
    client' = { ...doRetry(client, rid, np, faulted),
                retriedReservation: client.retriedReservation
                                      or not(client.pending.get(rid).inflight) },
    oracle' = oracle,
  }

  // The same failure with no eligible peer left, or with no slot left -> drop entry, rollback
  // syncHeight and leave the range to the next request pass.
  action retryUnserved(rid: ReqId, faulted: bool): bool = all {
    rid != -1,
    client.pending.keys().contains(rid),
    retryable(rid),
    or {
      budgetAfterRemoval(rid) >= maxParallelRequests,
      not(client.known.keys().exists(q => and {
        not(q.in(retryExcluded(client.pending.get(rid), faulted))),
        canPrefix(client.known.get(q), client.pending.get(rid).range),
      })),
    },
    client' = { ...doRetryRollback(client, rid, faulted),
                retriedReservation: client.retriedReservation
                                      or not(client.pending.get(rid).inflight) },
    oracle' = oracle,
  }

  // Consensus decides the next height once its value has been synced (fused decide+start).
  action decide = all {
    client.started,
    client.buffered.contains(client.tip + 1),
    client' = applyDecide(client),
    oracle' = oracle,
  }

  // Consensus can decide independently of ValueSync when the network has reached the next height.
  action decideFromConsensus = all {
    client.started,
    client.tip < TOP,
    oracle.keys().exists(p => oracle.get(p).tip >= client.tip + 1),
    client' = applyDecide(client),
    oracle' = oracle,
  }

  action step = {
    nondet p = PEERS.oneOf()
    nondet rid = client.pending.keys().union(Set(-1)).oneOf()
    nondet k = 1.to(BATCH_SIZE).oneOf()
    nondet advertisedTip = 1.to(TOP).oneOf()
    nondet faulted = Set(false, true).oneOf()
    any {
      startConsensus,
      requestValues(p),
      deliverFull(rid),
      deliverPartial(rid, k),
      retryServed(rid, p, faulted),
      retryUnserved(rid, faulted),
      decide,
      decideFromConsensus,
      peerAdvance(p),
      peerPrune(p),
      receiveStatus(p),
      receiveAdvertisedStatus(p, advertisedTip),
      peerDisconnect(p),
    }
  }

  // ============================================================================
  // Properties
  // ============================================================================
  // Safety invariants -----------------------------------------------------------

  // I1. syncHeight is always strictly above the tip (once consensus has started).
  val invSyncAboveTip: bool =
    client.started implies client.syncHeight > client.tip

  // I2. syncHeight never falls inside a pending request range.
  val invSyncUncovered: bool =
    not(coversHeight(client.pending, client.syncHeight))

  // I3. pending request ranges are pairwise disjoint.
  val invDisjoint: bool =
    client.pending.keys().forall(i =>
      client.pending.keys().forall(j =>
        (i == j) or rDisjoint(client.pending.get(i).range, client.pending.get(j).range)))

  // I4. no IN-FLIGHT entry is owned by a peer absent from the known set. A request that no peer
  // can answer must never stay outstanding. Reservations are exempt: their values already
  // arrived, so `peer` only records who served them, and the entry survives that peer leaving.
  val invNoOrphanedOwner: bool =
    client.pending.keys().forall(id =>
      client.pending.get(id).inflight
        implies client.known.keys().contains(client.pending.get(id).peer))

  // I5. every height strictly between tip and syncHeight is covered by a pending request.
  val invNoLostHeight: bool =
    client.started implies
      0.to(MAXH).forall(h =>
        (client.tip < h and h < client.syncHeight) implies coversHeight(client.pending, h))

  // I6. Completed reservations cannot exhaust capacity while useful frontier work is available.
  val invNoCapacityBlockedFrontier: bool = not(capacityBlockedFrontier)

  // I7. the in-flight requests never exceed the parallel-request budget.
  val invBudgetRespected: bool = inflightCount <= maxParallelRequests

  // Witnesses (reachability) -----------------------------------------------------
  val wStarted: bool = client.started
  val wPending: bool = client.pending.keys().size() > 0
  val wParallel: bool = inflightCount > 1
  val wPlaceholder: bool =
    client.pending.keys().exists(id => not(client.pending.get(id).inflight))
  val wRetried: bool =
    client.pending.keys().exists(id => client.pending.get(id).excluded.size() > 0)
  val wPrunedStatus: bool =
    client.known.keys().exists(p => client.known.get(p).historyMin > HISTORY_MIN)
  val wProgress: bool = client.tip >= 2
  val wCaughtUp: bool = client.started and client.tip == TOP
  val wCapacityBlockedFrontier: bool = capacityBlockedFrontier

  // A reservation was re-requested, so the `on_peer_fault` path above is reachable.
  val wRetriedReservation: bool = client.retriedReservation
  // Evidence the disconnect+reroute path fired: a pending entry carries a disconnected peer
  // (now absent from known) in its excluded set — only a reroute after disconnect/failure
  // produces this.
  val wReroutedPastDisconnected: bool =
    client.pending.keys().exists(id =>
      client.pending.get(id).excluded.exists(q => not(client.known.keys().contains(q))))
}

module vs {
  import valuesync(
    PEERS = Set("p1", "p2", "p3"),
    BATCH_SIZE = 2,
    MAX_PARALLEL = 2,
    TOP = 4,
    HISTORY_MIN = 1
  ).*
}

// Larger instance for stress-testing the invariants.
module vsBig {
  import valuesync(
    PEERS = Set("p1", "p2", "p3", "p4"),
    BATCH_SIZE = 3,
    MAX_PARALLEL = 3,
    TOP = 6,
    HISTORY_MIN = 1
  ).*
}

// ============================================================================
// SCOPE AND CORRESPONDENCE TO THE CODE
// ============================================================================
//
// SOURCE-FILE CORRESPONDENCE
//   valuesync.qnt  <->  code/crates/sync/src/handle.rs   (client + server handlers)
//                       code/crates/sync/src/state.rs    (State, peer/range selection)
//                       code/crates/engine/src/sync.rs   (buffering by consensus_height,
//                                                          decide/started-height wiring)
//   Update this model and re-check its invariants BEFORE changing those files.
//
// WHAT THIS MODEL COVERS
//   - The client-side sync frontier: sync_height, pending_requests (inflight + placeholder
//     reservations), tip/consensus_height, buffered synced values.
//   - Proactive parallel batch requests (request_values / find_next_uncovered_range_from), gated
//     by the in-flight request count and the read-ahead window (see `frontierRequestable`).
//   - Response handling: full, partial-prefix (the suffix returns to the frontier), and retry with
//     peer exclusion (unified as `retry*`). A retry covers an in-flight entry that failed through
//     an invalid response or a timeout, and the reservation holding `consensusHeight` when it
//     failed through `on_peer_fault` or `on_local_transient_error`. The `faulted` flag separates
//     the two triggers: only a peer-attributable failure drops the buffered siblings of the
//     retried range and bars the serving peer from the retry.
//   - Peer selection over Status (whole-range preferred, prefix fallback), with peer tips and
//     pruning floors changing independently of the client's last-known view. The known-peer set
//     (state.peers) grows or updates on Status (receiveStatus) and shrinks on disconnect.
//   - Peer disconnect (PeerDisconnected): drops the peer from the known set; its in-flight
//     ranges are re-requested through the shared retry primitive (see the retry* actions).
//   - Consensus decide + start-height, pruning, and the sync_height re-validation/rollback
//     machinery (set_sync_height).
//
// VERIFIED (quint run, sampled; instances `vs` and `vsBig`)
//   HOLD: invSyncAboveTip, invSyncUncovered, invDisjoint, invNoOrphanedOwner, invNoLostHeight,
//         invNoCapacityBlockedFrontier.
//
// WHAT THIS MODEL DOES NOT COVER (deliberate)
//   - Server side (on_value_request / on_got_decided_values): abstracted into the oracle,
//     which returns a valid prefix, an invalid response, or times out.
//   - EMA peer scoring: replaced by nondeterministic selection among eligible peers.
//   - Transport-failure rollback (SendValueRequest -> None): its state effect is identical to
//     the modeled no-eligible-peer rollback, so it is covered in spirit, not as its own action.
//   - CancelValueRequest effect: pure network-layer cleanup (drop an
//     abandoned request), no effect on sync state, so it is not modeled.
//   - on_peer_disconnected re-requests ALL of a gone peer's in-flight ranges; the model does
//     this ATOMICALLY (peerDisconnect folds the retry primitive over every affected range in one
//     action) but reroutes them to a single replacement peer rather than one-per-range.
//   - Consensus-reported InvalidValue/ValueProcessingError on ALREADY-buffered values, and the
//     StartedHeight(Restart) path (clears pending, resets sync_height).
//   - The engine's transport-request map (`state.inflight` in engine/src/sync.rs). `pending` here
//     stands for what ValueSync tracks, and the two can diverge: a restart clears
//     `pending_requests` and pruning drops entries, neither of which ends the transport request.
//     `invBudgetRespected` therefore bounds tracked requests, not live wire requests.
//   - Multi-node consistency/agreement: this is a single-client model; values are canonical.
//   - Liveness as a temporal property; catch-up is checked only as a reachability witness
//     (wCaughtUp).
//   - THE BUDGET RULE ON THE RETRY PATH, under sampled search. `invBudgetRespected` is sound, but
//     the state that breaks it needs a narrow interleaving: a one-height reservation, a full
//     budget above it, and a second known peer eligible for its range. Removing the budget guard
//     from `retryServed` survives 500,000 samples at 40 steps on `vs`. The guard is pinned by
//     `reservationRetryStaysInsideBudgetTest` and by the Rust test
//     `test_re_request_respects_the_parallel_request_budget`, not by the search.
//   - WHICH INPUTS TRIGGER A REQUEST PASS. `requestValues` fires whenever its guard holds, but
//     the implementation runs `request_values` only from `on_status`, `on_started_height` and
//     after a response releases a slot. A pass that the code never triggers therefore still
//     looks reachable here, so this model cannot detect a missing trigger. Every invariant
//     below assumes the code calls `request_values` wherever `requestValues` is enabled;
//     changing a call site needs a Rust test, not a model check.
// ============================================================================
