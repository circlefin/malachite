// -*- mode: Bluespec; -*-

// Instance for the positive agreement witness. n=4, f=1 (within n > 3f).
// Run by CI via `quint test` (.github/workflows/quint.yml runs every *Test.qnt).

module agreementTest {

import agreementRun(
    validators   = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty       = Set("v2"),                 // 1 of 4 faulty => within bound;
                                              // v2 is the round-0 proposer (driver.qnt:223),
                                              // so the proposed value is deliverable via the
                                              // faulty-proposal path (mirrors disagreementRun).
    Values       = Set("a", "b"),
    Rounds       = Set(0),
    Heights      = Set(0)
) as N4F1 from "./agreementRun"

run agreementHoldsTest = {
    N4F1::AgreementRun
}

}
