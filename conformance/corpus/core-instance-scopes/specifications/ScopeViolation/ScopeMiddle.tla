---- MODULE ScopeMiddle ----
EXTENDS Naturals
VARIABLE w
I == INSTANCE ScopeInner WITH v <- w + 1
Double == I!Double
====
