---- MODULE MulMiddle ----
EXTENDS Naturals
VARIABLE w
I == INSTANCE MulInner WITH v <- w + 1
Double == I!Left
====
