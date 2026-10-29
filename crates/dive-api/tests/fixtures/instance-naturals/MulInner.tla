---- MODULE MulInner ----
EXTENDS Naturals
VARIABLE v
Left == 2 * v
Right == v * 2
Nested == 2 * (v + 1)
Pair == <<v, v>>
Plus == v + 5
====
