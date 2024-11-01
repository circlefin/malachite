# Malachite Consensus

Malachite is an implementation of the [Tendermint consensus algorithm][tendermint-arxiv] in Rust.
It comes together with an executable specification in [Quint][quint-spec].
We use model-based testing to make sure that the implementation corresponds to
the specification.

The specification of the consensus algorithm and its implementation is organized as follows:

- [**overview.md**](./overview.md): a summary of the operation and components
  of the Tendermint consensus algorithm;
- [**pseudo-code.md**](./pseudo-code.md): the Algorithm in page 6 of the
  Tendermint [paper][tendermint-pdf];
  since it is referenced several times in this specification, for simplicity and
  easy reference it was copied into this file;
- [**misbehavior.md**](./misbehavior.md): describes different kinds of
  misbehavior by Byzantine processes that over time can harm the system (lead to
  disagreement), and how each misbehavior is defined and can be detected;

## References

- ["The latest gossip on BFT consensus"][tendermint-arxiv],
  by _Ethan Buchman, Jae Kwon, and Zarko Milosevic_. 2018.
  ([PDF][tendermint-pdf])

[accountable-tendermint]: ./misbehavior.md#misbehavior-detection-and-verification-in-accountable-tendermint
[tendermint-arxiv]: https://arxiv.org/abs/1807.04938
[tendermint-pdf]: https://arxiv.org/pdf/1807.04938
[quint-spec]: ../quint/README.md
