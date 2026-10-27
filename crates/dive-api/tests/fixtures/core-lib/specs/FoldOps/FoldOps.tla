---- MODULE FoldOps ----
\* FiniteSetsExt's FoldSet and Functions' FoldFunctionOnSet in
\* crates/dive-core/src/stdlib/higher.rs: the pinned TLC folds every set in
\* its normalized order, without repeats.
EXTENDS Naturals, Sequences, FiniteSetsExt, Functions
Cnt(e, acc) == acc + 1
Cat(e, acc) == Append(acc, e)
Sum(e, acc) == e + acc
VARIABLE s
Init == s = {1}
Next == \/ Cardinality(s) < 4 /\ \E x \in 1..5 : s' = s \cup {x}
        \/ s' = {FoldSet(Sum, 0, s) % 5}
Inv == /\ FoldSet(Cnt, 0, s) = Cardinality(s)
       /\ FoldSet(Cnt, 0, s \cup {1, 2}) = Cardinality(s \cup {1, 2})
       /\ FoldSet(Cnt, 0, UNION {s, {2, 3}}) = Cardinality(s \cup {2, 3})
       /\ Len(FoldSet(Cat, <<>>, s)) = Cardinality(s)
       /\ FoldSet(Cat, <<>>, {3, 1} \cup {2, 0}) = <<0, 1, 2, 3>>
       /\ FoldSet(Cat, <<>>, {3, 1, 2} \ {1}) = <<2, 3>>
       /\ FoldSet(Cat, <<>>, {3 - i : i \in 1..3}) = <<0, 1, 2>>
       /\ FoldSet(Cat, <<>>, SUBSET {1, 2, 3}) = <<{}, {1}, {2}, {3}, {1, 2}, {1, 3}, {2, 3}, {1, 2, 3}>>
       /\ FoldSet(Cat, <<>>, {1, 2} \X {3, 4}) = <<<<1, 3>>, <<1, 4>>, <<2, 3>>, <<2, 4>>>>
       /\ FoldSet(Cat, <<>>, [{1, 2} -> {3, 4}]) = <<<<3, 3>>, <<3, 4>>, <<4, 3>>, <<4, 4>>>>
       /\ FoldSet(Cat, <<>>, {"b", "a"}) = <<"a", "b">>
       /\ FoldSet(Sum, 7, {}) = 7
       /\ FoldFunctionOnSet(Cat, <<>>, [i \in 1..4 |-> 10 * i], {3, 1}) = <<10, 30>>
       /\ FoldFunctionOnSet(Sum, 0, [i \in 0..5 |-> i * i], s) = FoldSet(LAMBDA e, acc : e * e + acc, 0, s)
       /\ FoldFunctionOnSet(Cat, <<>>, [a |-> 1, b |-> 2], {"b", "a"}) = <<1, 2>>
====
