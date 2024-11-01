# Consensus Algorithm

Malachite adopts the Tendermint consensus algorithm from the paper
["The latest gossip on BFT consensus"][tendermint-arxiv]
([PDF][tendermint-pdf]), by Ethan Buchman, Jae Kwon,
and Zarko Milosevic, last revised in November 2019.

The [**overview.md**](./overview.md) document presents a summary of the
operation and components of the Tendermint consensus algorithm.

The [**pseudo-code.md**](./pseudo-code.md) document presents the Algorithm
in page of the Tendermint [paper][tendermint-pdf].
Since it is referenced several times in this specification, for simplicity and
easy reference it was copied into this file.

[accountable-tendermint]: ./misbehavior.md#misbehavior-detection-and-verification-in-accountable-tendermint
[tendermint-arxiv]: https://arxiv.org/abs/1807.04938
[tendermint-pdf]: https://arxiv.org/pdf/1807.04938
