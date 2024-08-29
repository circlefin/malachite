// -*- mode: Bluespec; -*-

module binary_heap {
    import spells.* from "./spells"

    type BinaryHeap[a] = List[(int, a)]

    pure def empty(): BinaryHeap[a] = []

    pure def isEmpty(heap: BinaryHeap[a]): bool =
        heap.length() == 0

    pure def compare(l: (int, a), r: (int, a)): Ordering = 
        intCompare(l._1, r._1)

    pure def insert(heap: BinaryHeap[a], value: (int, a)): BinaryHeap[a] = {
        heap.sortedListInsert(value, compare)
    }

    pure def peek(heap: BinaryHeap[a]): Option[a] = {
        if (isEmpty(heap)) {
            None
        } else {
            Some(heap[0]._2)
        }
    }

    pure def pop(heap: BinaryHeap[a]): (BinaryHeap[a], Option[a]) = {
        if (isEmpty(heap)) {
            (heap, None)
        } else {
            (heap.tail(), Some(heap[0]._2))
        }
    }

    pure def drop(heap: BinaryHeap[a]): BinaryHeap[a] = {
        if (isEmpty(heap)) {
            heap
        } else {
            heap.tail()
        }
    }

    run heapTest = {
        val a = (1, "a")
        val b = (2, "b")
        val c = (3, "c")
        val d = (4, "d")

        all {
            assert(List().peek() == None),
            assert(List().pop() == (List(), None)),

            assert(List().insert(b) == List(b)),
            assert(List(b).peek() == Some("b")),

            assert(List(b).insert(a) == List(a, b)),
            assert(List(a, b).peek() == Some("a")),
            assert(List(a, b).insert(d) == List(a, b, d)),

            assert(List(a, b, d).insert(c) == List(a, b, c, d)),
            assert(List(a, b, d).pop() == (List(b, d), Some("a"))),
        }
    }
}

