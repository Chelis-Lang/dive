---- MODULE Bits ----
\* The placeholder body is never evaluated: TLC loads Bits.class (the pinned
\* Examples Hanoi class) from this directory and uses its Java method
\* Bits.And(x, y, n, m) == x & y on 32-bit ints instead.
And(x, y, n, m) == 99
x & y == And(x, y, 0, x)
====
