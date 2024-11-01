# Consensus Algorithm

Malachite adopts the Tendermint consensus algorithm from the paper
["The latest gossip on BFT consensus"][tendermint-arxiv]
([PDF][tendermint-pdf]), by Ethan Buchman, Jae Kwon,
and Zarko Milosevic, last revised in November 2019.

Refer to the [**Overview**][overview] document for a summary of the
operation and components of the Tendermint consensus algorithm.
The **pseudo-code** of the algorithm, referenced several times in this
specification, is the Algorithm in [page 6][tendermint-pdf], that for simplicity and easy
reference is presented in the [pseudo-code.md][pseudo-code] file.

[overview]: ./overview.md
[pseudo-code]: ./pseudo-code.md
[accountable-tendermint]: ./misbehavior.md#misbehavior-detection-and-verification-in-accountable-tendermint
[tendermint-arxiv]: https://arxiv.org/abs/1807.04938
[tendermint-pdf]: https://arxiv.org/pdf/1807.04938
