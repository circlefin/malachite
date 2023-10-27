// -*- mode: Bluespec; -*-
/**
 * This module collects definitions that are ubiquitous.
 * One day they will become the standard library of Quint.
 *
 * Manuel Bravo, Informal Systems, 2023
 */
module extraSpells {

  import basicSpells.* from "./basicSpells"

  pure def printVariable(name: str, variable: a): bool = true

  pure def printMapVariables(__map: a -> b): bool = true

  /// Compute the minimum of two integers.
  ///
  /// - @param __i first integer
  /// - @param __j second integer
  /// - @returns the minimum of __i and __j
  pure def min(__i: int, __j: int): int = {
    if (__i > __j) __j else __i
  }

  run minTest = all {
    assert(min(3, 4) == 3),
    assert(min(6, 3) == 3),
    assert(min(10, 10) == 10),
    assert(min(-3, -5) == -5),
    assert(min(-5, -3) == -5),
  }

  /// Compute the max of a set of integers.
  /// The set must be non-empty
  ///
  /// - @param __set a set of integers
  /// - @returns the max
  pure def maxSet(__set: Set[int]): int = {
    __set.fold((true, 0), (__max, __i) => if (__max._1) (false, __i)
                                          else if (__i > __max._2) (false, __i) else (false, __max._2))._2
  }

  run maxSetTest = all {
    assert(maxSet(Set(3, 4, 8)) == 8),
    assert(maxSet(Set(3, 8, 4)) == 8),
    assert(maxSet(Set(8, 4, 3)) == 8),
    assert(maxSet(Set(8, 8, 8)) == 8),
    assert(maxSet(Set(-3, -4, -8)) == -3),
    assert(maxSet(Set(-3, -8, -4)) == -3),
    assert(maxSet(Set(-8, -4, -3)) == -3),
  }

    /// Compute the min of a set of integers.
  /// The set must be non-empty
  ///
  /// - @param __set a set of integers
  /// - @returns the min
  pure def minSet(__set: Set[int]): int = {
    __set.fold((true, 0), (__min, __i) => if (__min._1) (false, __i)
                                          else if (__i < __min._2) (false, __i) else (false, __min._2))._2
  }

  run minSetTest = all {
    assert(minSet(Set(3, 4, 8)) == 3),
    assert(minSet(Set(3, 8, 4)) == 3),
    assert(minSet(Set(8, 4, 3)) == 3),
    assert(minSet(Set(8, 8, 8)) == 8),
    assert(minSet(Set(-3, -4, -8)) == -8),
    assert(minSet(Set(-3, -8, -4)) == -8),
    assert(minSet(Set(-8, -4, -3)) == -8),
  }

  /// Orders a set of integers in decreasing order.
  ///
  /// - @param __set a set of integers
  /// - @returns a list with the integers ordered in decreasing order
  pure def sortSetDecreasing(__set: Set[int]): List[int] = {
    __set.fold({unordered: __set, ordered: List()}, (acc, _) => val __max = maxSet(acc.unordered)
                                                    {unordered: acc.unordered.setRemove(__max), ordered: acc.ordered.append(__max)}).ordered
  }

  run sortSetDecreasingTest = all {
    assert(sortSetDecreasing(Set(3, 8, 4)) == [8, 4, 3]),
    assert(sortSetDecreasing(Set(8, 4, 3)) == [8, 4, 3]),
    assert(sortSetDecreasing(Set()) == []),
    assert(sortSetDecreasing(Set(9, -2, 5, 10)) == [10, 9, 5, -2]),
  }

  /// Orders a set of integers in increasing order.
  ///
  /// - @param __set a set of integers
  /// - @returns a list with the integers ordered in increasing order
  pure def sortSetIncreasing(__set: Set[int]): List[int] = {
    __set.fold({unordered: __set, ordered: List()}, (acc, _) => val __max = minSet(acc.unordered)
                                                    {unordered: acc.unordered.setRemove(__max), ordered: acc.ordered.append(__max)}).ordered
  }

  run sortSetIncreasingTest = all {
    assert(sortSetIncreasing(Set(3, 8, 4)) == [3, 4, 8]),
    assert(sortSetIncreasing(Set(8, 4, 3)) == [3, 4, 8]),
    assert(sortSetIncreasing(Set()) == []),
    assert(sortSetIncreasing(Set(9, -2, 5, 10)) == [-2, 5, 9, 10]),
  }

  /// Add a set element.
  /// - @param __set a set to add an element to
  /// - @param __elem an element to add
  /// - @returns a new set that contains all elements of __set and __elem
  pure def setAdd(__set: Set[a], __elem: a): Set[a] = {
    __set.union(Set(__elem))
  }

  run setAddTest = all {
    assert(Set(2, 3, 4, 5) == Set(2, 3, 4).setAdd(5)),
    assert(Set(3) == Set(3).setAdd(3)),
  }

  /// Safely set a map entry.
  ///
  /// - @param __map a map to set an entry to
  /// - @param __key the key of an entry to set
  /// - @returns a new map that contains all entries of __map
  ///          and an entry for the key __key
  pure def mapSafeSet(__map: a -> b, __key: a, __value: b): a -> b = {
    if (__map.has(__key)) {
      __map.set(__key, __value)
    } else {
      __map.keys().union(Set(__key)).mapBy(__k => if (__k == __key) __value else __map.get(__k))
    }
  }

  run mapSafeSetTest = all {
    assert(Map(2 -> 3, 4 -> 5).mapSafeSet(1, 7) == Map(1 -> 7, 2 -> 3, 4 -> 5)),
    assert(Map(2 -> 3, 4 -> 5).mapSafeSet(2, 7) == Map(2 -> 7, 4 -> 5))
  }

  /// Remove a set of map entries. 
  ///
  /// - @param __map a map to remove the set of entries from
  /// - @param __keys the set of keys to remove
  /// - @returns a new map that contains all entries of __map
  ///          but the ones in __keys
  pure def mapRemoveSet(__map: a -> b, __keys: Set[a]): a -> b = {
    __map.keys().exclude(__keys).mapBy(__k => __map.get(__k))
  }

  run mapRemoveSetTest = all {
    assert(Map(2 -> 3, 4 -> 5, 7 -> 8).mapRemoveSet(Set(2, 7)) == Map(4 -> 5)),
    assert(Map(2 -> 3, 4 -> 5, 7 -> 8).mapRemoveSet(Set(1, 7)) == Map(2 -> 3, 4 -> 5)),
    assert(Map(2 -> 3, 4 -> 5, 7 -> 8).mapRemoveSet(Set(1, 3)) == Map(2 -> 3, 4 -> 5, 7 -> 8)),
    assert(Map(2 -> 3, 4 -> 5, 7 -> 8).mapRemoveSet(Set(2, 4, 7)) == Map()),
  }

  /// Add a set of map entries to a map, add if entry exists.
  /// The values must be integers
  ///
  /// - @param __map a map to add the set of entries to
  /// - @param __entries the set of new entries to add
  /// - @returns a new map that contains all entries of __map
  ///          plus the ones in __entries
  pure def mapAddSet(__map: a -> int, __entries: a -> int): a -> int = {
    __map.keys().union(__entries.keys()).mapBy(__k => if (__map.has(__k) and __entries.has(__k)) __map.get(__k) + __entries.get(__k)
                                                       else if (__map.has(__k)) __map.get(__k) else __entries.get(__k))
  }

  run mapAddSetTest = all {
    assert(Map(2 -> 3, 4 -> 5, 7 -> 8).mapAddSet(Map(3 -> 6, 8 -> 9)) == Map(2 -> 3, 4 -> 5, 3 -> 6, 7 -> 8, 8 -> 9)),
    assert(Map(2 -> 3, 4 -> 5, 7 -> 8).mapAddSet(Map(3 -> 6, 7 -> 9)) == Map(2 -> 3, 4 -> 5, 3 -> 6, 7 -> 17)),
  }
}