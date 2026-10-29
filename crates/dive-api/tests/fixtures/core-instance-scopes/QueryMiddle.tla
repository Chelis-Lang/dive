---- MODULE QueryMiddle ----
EXTENDS Naturals
CONSTANT limit
VARIABLE x
I == INSTANCE QueryInner WITH limit <- limit, x <- x
Can == I!Can
====
