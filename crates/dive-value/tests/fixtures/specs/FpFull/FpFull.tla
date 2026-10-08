---- MODULE FpFull ----
EXTENDS Integers, Sequences, FiniteSets, TLC
VARIABLES i, x
V == << 0, 1, -1, 2147483647, -2147483647, TRUE, FALSE, "qsq", "", "qé中", "ā",
        {3, 1, 2}, {}, 1..3, <<1, 2>>, <<>>, [qfa |-> 1, qfb |-> "qsr"],
        (5 :> TRUE @@ 7 :> FALSE), {<<1, {2}>>, <<3, {}>>}, SUBSET {1, 2},
        [{1, 2} -> {3}], {1, 2} \X {3}, [qfa : {1, 2}], {"qss", "qst"},
        (2 :> 1 @@ 3 :> 2), ({1} :> 2), 3..2, <<<<>>>> >>
N == Len(V)
Init == i = 1 /\ x = V[1]
Next == i < N /\ i' = i + 1 /\ x' = V[i + 1]
====
