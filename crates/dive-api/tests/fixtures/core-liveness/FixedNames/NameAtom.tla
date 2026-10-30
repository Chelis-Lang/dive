---- MODULE NameAtom ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x=37
Next == UNCHANGED x
P == <>[](ToString(x+1)#"never")
====
