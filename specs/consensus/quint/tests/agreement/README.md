# Agreement (step 1 of implementation verification)

This directory is the start of **step 1**: showing that Malachite's *own* Quint
model satisfies **agreement** — no two correct validators ever decide differently
at the same height (`statemachineAsync::AgreementInv`).

Why this matters: MBT (`code/crates/test/mbt/`) already replays Quint traces
through the Rust, but the Quint model itself was never shown to *imply* agreement.
Before `AgreementInv` lived here, the only place it appeared was
`tests/disagreement/disagreementRun.qnt`, which asserts it is **false** under
`f = 2` (a fork). Nothing asserted it **holds**.

## What is here now

| File | What it is | How it runs | Status |
|------|------------|-------------|--------|
| `agreementRun.qnt` | Positive witness: `n=4, f=1`, the correct quorum decides one value | `quint test` (CI) | ✅ passing, non-vacuous (all correct decide) |
| `agreementTest.qnt` | Instance of the above (CI runs every `*Test.qnt`) | `quint test --max-samples 100` | ✅ passing |
| `agreementInductive.qnt` | Apalache harness exposing `init/step/AgreementInv` | `quint verify` (manual; needs Apalache) | ⏳ harness OK; inductive invariant TODO |

The positive run and the existing `disagreementRun` now bracket the threshold:
`f = 1` (within `n > 3f`) → agreement holds; `f = 2` (beyond it) → fork. Both are
single-trace witnesses checked by randomized `quint test`, **not** proofs.

```sh
# positive witness (this dir), CI-equivalent:
quint test specs/consensus/quint/tests/agreement/agreementTest.qnt --max-samples 100
# negative witness (existing):
quint test specs/consensus/quint/tests/disagreement/disagreementTest.qnt
```

## What is still missing: the proof

`AgreementInv` for *all* executions needs an **inductive invariant** `IndInv` with
the standard obligations (cf. era-consensus `simplified/`):

1. `Init ⇒ IndInv`
2. `IndInv ⇒ AgreementInv`
3. `IndInv ∧ step ⇒ IndInv'`

checked bounded-but-all-executions with Apalache:

```sh
quint verify --max-steps 1 --init init  --invariant IndInv          agreementInductive.qnt
quint verify --max-steps 1 --init init  --invariant AgreementInv    agreementInductive.qnt   # IndInv ⇒ Agreement
quint verify --max-steps 1 --init indInit --step step --invariant IndInv agreementInductive.qnt # inductive step
```

(Free model-checking from `init` is *not* the path: Apalache stays load/solver-bound
on the full async model before any decision is reached — confirmed locally. The
inductive route is what makes it tractable.)

### Porting plan for `IndInv`

The lemmas are a direct port of Konnov's **already-machine-checked** Tendermint
inductive invariant
`cometbft/spec/light-client/accountability/TendermintAccInv_004_draft.tla`
(the `TypedInv` whose theorem `LessThanThirdFaulty ∧ TypedInv ⇒ Agreement`). Galois's
parametric Ivy version (`cometbft/spec/ivy-proofs/classic_safety.ivy`, invariant
`locks`) is the cross-check. Mapping onto Malachite's async state
(`system : Address -> NodeState`, plus `voteBuffer`/`propBuffer`):

| Konnov lemma (TLA+) | Malachite analog | Notes |
|---|---|---|
| `AllLockedRoundIffLockedValue` | `lockedRound = -1 ⟺ lockedValue = Nil` over `system.get(v).es.cs` | direct |
| `AllNoEquivocationByCorrect` | a correct `v` sends ≤1 prevote / ≤1 precommit per `(h,r)` | needs ghost msg-history |
| `AllIfInPrecommitThenSentPrecommit` / `IfSentPrecommitThenSentPrevote` | step ⇒ corresponding message sent | needs ghost msg-history |
| `AllIfLockedRoundThenSentCommit` / `AllLatestPrecommitHasLockedRound` | locked value/round tracks latest precommit sent | needs ghost msg-history |
| `IfSentPrevoteThenReceivedProposalOrTwoThirds` | a correct prevote is justified by a proposal (+ 2/3 in a valid round) | core locking support |
| **`PrecommitsLockValue`** | if `f+1` precommit `v` in round `r`, no `2f+1` prevotes for `≠v` in later rounds | **the safety kernel** — hardest, but already proven upstream |

**Prerequisite instrumentation:** the async model consumes messages out of the
buffers, so the lemmas above need a monotonic **ghost set of all messages ever
sent** added to `statemachineAsync.qnt` (the analog of era's `ghost_commit_qc` /
Konnov's persistent `msgsPrevote`/`msgsPrecommit`). That ghost state plus the ~11
safety-relevant lemmas (the `validValue`/`validRound` lemmas are "UNUSED FOR
SAFETY" upstream) is the bulk of the remaining work. Estimate: a few weeks for
someone fluent in Quint/Apalache, dominated by re-homing + the ghost instrumentation,
since the hard lemma bodies already exist upstream.
