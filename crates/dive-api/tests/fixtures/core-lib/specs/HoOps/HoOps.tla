---- MODULE HoOps ----
\* Library operators with operator arguments (crates/dive-core/src/stdlib/
\* higher.rs), passed as LAMBDAs, definitions, LET operators and operator
\* parameters, in actions and invariants.
EXTENDS Naturals, Sequences, TLC, Bags, BagsExt, FiniteSetsExt, Functions, SequencesExt
VARIABLE s
Even(x) == x % 2 = 0
Lt(a, b) == a < b
Add(a, b) == a + b
Count(S, P(_)) == Quantify(S, P)
Init == s \in {<<>>, <<3, 1>>, <<2, 2, 1>>}
Next == \/ Len(s) < 4 /\ \E x \in 1..3 : s' = Append(s, x)
        \/ s # <<>> /\ s' = SelectSeq(s, LAMBDA x : x # s[1])
        \/ s' = SortSeq(s, Lt)
Inv == /\ SelectSeq(s, Even) = SelectSeq(s, LAMBDA x : ~(x % 2 = 1))
       /\ Len(SortSeq(s, LAMBDA a, b : a > b)) = Len(s)
       /\ LET Gt(a, b) == a > b IN SortSeq(<<2, 3, 1>>, Gt) = <<3, 2, 1>>
       /\ FoldLeft(Add, 0, s) = FoldRight(Add, s, 0)
       /\ FoldLeft(LAMBDA a, x : a \o <<x>>, <<>>, s) = s
       /\ FoldRight(LAMBDA x, a : <<x>> \o a, s, <<>>) = s
       /\ FoldLeftDomain(LAMBDA a, i : a + i, 0, s) = (Len(s) * (Len(s) + 1)) \div 2
       /\ FoldFunction(Add, 0, s) = FoldSeq(Add, 0, s)
       /\ FoldFunction(LAMBDA v, a : a + v, 0, [k \in {"a", "b"} |-> 2]) = 4
       /\ Count(1..10, Even) = 5
       /\ Quantify(ToSet(s), LAMBDA x : x > 1) = Cardinality({x \in ToSet(s) : x > 1})
       /\ SelectInSeq(s, Even) = (IF \E i \in 1..Len(s) : Even(s[i])
                                  THEN CHOOSE i \in 1..Len(s) : Even(s[i]) /\ \A j \in 1..i - 1 : ~Even(s[j])
                                  ELSE 0)
       /\ SelectLastInSeq(<<2, 1, 2>>, Even) = 3
       /\ RemoveFirstMatch(<<1, 2, 3, 2>>, Even) = <<1, 3, 2>>
       /\ BagOfAll(LAMBDA x : x % 2, SetToBag(1..5)) = (0 :> 2 @@ 1 :> 3)
       /\ BagCardinality(BagOfAll(LAMBDA x : x * 0, SetToBag(ToSet(s)))) = Cardinality(ToSet(s))
       /\ FoldBag(LAMBDA a, x : a + x, 0, <<2, 1>>) = 4
       /\ FoldBag(Add, 0, EmptyBag) = 0
====
