// -*- mode: Bluespec; -*-

// ===========================================================================
// Step 1 of the Malachite verification effort: model-level AGREEMENT.
//
// Goal: show that `statemachineAsync::AgreementInv` (no two correct validators
// ever decide differently at the same height) HOLDS on Malachite's own async
// Quint model -- the model that MBT already replays through the Rust.
//
// Status (2026-06-06): SCAFFOLD + bounded reachability check.
//   - This module instantiates the async model at n=4, f=1 (within n > 3f) and
//     exposes init/step/AgreementInv so Apalache can check it via `quint verify`.
//   - The repo previously only ever asserted AgreementInv *false* (in the
//     disagreement run, f=2). This is the first harness that checks it *holds*.
//   - The full UNBOUNDED result needs the inductive invariant `IndInv` below,
//     ported from CometBFT's `TendermintAccInv_004_draft.tla` (Konnov, 2020).
//     That port is in progress -- see the lemma stubs and the README.
//
// Provenance of the inductive approach:
//   cometbft/spec/light-client/accountability/TendermintAccInv_004_draft.tla
//   era-consensus/spec/protocol-spec/simplified/inductive.qnt
// ===========================================================================

module agreementInductive {

  import types.* from "../../types"
  import extraSpells.* from "../../spells/extra"

  // ----- Instance: n = 4, f = 1, so n > 3f holds (agreement should be safe) ---
  import statemachineAsync(
    validators   = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty       = Set("v1"),                 // 1 of 4 faulty  => within bound
    Values       = Set("a", "b"),
    Rounds       = Set(0, 1),
    Heights      = Set(0)
  ) as M from "../../statemachineAsync"

  // Delegate init/step/invariant to the instance so `quint verify` can drive it.
  action init = M::init
  action step = M::step

  // The property we want to prove holds (currently checked by bounded MC).
  val AgreementInv = M::AgreementInv

}
