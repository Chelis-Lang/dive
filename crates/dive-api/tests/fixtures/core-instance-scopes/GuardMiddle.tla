---- MODULE GuardMiddle ----
EXTENDS Naturals
VARIABLE w
Offset == w + 1
I == INSTANCE GuardInner WITH v <- Offset
Double == I!Double
====
