---- MODULE ConstructorAlias ----
EXTENDS Naturals
VARIABLE x

\* The sole initial occurrence is a nonliteral string-domain function.
Init == x = [k \in {"a"} |-> 0]
\* The sole generated occurrence is its equal-key record-literal representation.
Next == x' = [a |-> 0]

ActionCheck == <>[][~(x' = <<0, 0>>)]_x
StateControl == <>[]~(x = <<0, 0>>)

EventuallyEqual == <>(x = <<0, 0>>)
NextFunctionFirst ==
    \/ x' = [k \in {"a"} |-> 0]
    \/ x' = [a |-> 0]
NextRecordFirst ==
    \/ x' = [a |-> 0]
    \/ x' = [k \in {"a"} |-> 0]

EliminatedAction == <>(TRUE \/ <>[][~(x' = <<0, 0>>)]_x)
====
