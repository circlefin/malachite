// -*- mode: Bluespec; -*-

module spells {

  type Option[a] =
    | Some(a)
    | None

  pure def toSet(__list: List[a]): Set[a] = {
    __list.foldl(Set(), (__s, __e) => __s.union(Set(__e)))
  }

  pure def require(__cond: bool): bool = __cond

  /// The type of orderings between comparable things
  // Follows https://hackage.haskell.org/package/base-4.19.0.0/docs/Data-Ord.html#t:Ordering
  // and we think there are likely benefits to using 3 constant values rather than the more
  // common integer range in Apalache.
  type Ordering =
    | EQ
    | LT
    | GT

  pure def intCompare(a: int, b: int): Ordering = {
    if      (a < b) { LT }
    else if (a > b) { GT }
    else            { EQ }
  }

  /// Assuming `l` is sorted according to `cmp`, returns a list with the element `x`
  /// inserted in order.
  ///
  /// If `l` is not sorted, `x` will be inserted after the first element less than
  /// or equal to it.
  ///
  /// - @param l a sorted list
  /// - @param x an element to be inserted
  /// - @param cmp an operator defining an `Ordering` of the elements of type `a`
  /// - @returns a sorted list that includes `x`
  pure def sortedListInsert(l: List[a], x: a, cmp: (a, a) => Ordering): List[a] = {
    // We need to track whether x has been inserted, and the accumulator for the new list
    val init = { is_inserted: false, acc: List() }

    val result = l.foldl(init, (state, y) =>
      if (state.is_inserted)
        { ...state, acc: state.acc.append(y) }
      else
        match cmp(x, y) {
          | GT => { ...state, acc: state.acc.append(y) }
          | _  => { is_inserted: true, acc: state.acc.append(x).append(y)  }
        })

    if (not(result.is_inserted))
      // If x was not inserted, it was GT than every other element, so it goes at the end
      result.acc.append(x)
    else
      result.acc
  }
}


