---- MODULE DelayedClass ----
EXTENDS Naturals
VARIABLES x, n
Init == x = [a |-> 0] /\ n = 0
GeneralInit == x = [i \in {"a"} |-> 0] /\ n = 0
Next == n' = 1 /\ UNCHANGED x
I == n = 0 \/ ~(x = <<0, 0>>)
P == <>[]I
AliasInit == x = [a |-> 0] /\ n \in 0..15
AliasGeneralInit == x = [i \in {"a"} |-> 0] /\ n \in 0..15
AliasNext == (n' = 15 /\ x' = [a |-> 0]) \/ (n < 16 /\ n' = n + 16 /\ UNCHANGED x)
AliasI == n < 16 \/ ~(x = <<0, 0>>)
AliasP == <>[]AliasI
====
