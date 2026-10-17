---- MODULE HeldEnabled ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=1
A == ENABLED (LET held==x'=1 IN held) /\ x'=1
P == <>[][A]_x
Good == ENABLED (LET held==x'=1 IN held)
Bad == ~Good
====
