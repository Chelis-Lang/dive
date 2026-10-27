---- MODULE BagOps ----
\* The Bags operators of crates/dive-core/src/stdlib/bags.rs in actions
\* and invariants: \oplus, \ominus, BagUnion, \sqsubseteq, IsABag and
\* BagCardinality, over bags built as functions and tuples.
EXTENDS Naturals, Bags
VARIABLE b
One(x) == SetToBag({x})
Init == b \in {EmptyBag, One(1), [x \in {1, 2} |-> 2]}
Next == \/ BagCardinality(b) < 4 /\ b' = b (+) One(1)
        \/ b' = b (-) One(2)
        \/ BagCardinality(b) < 4 /\ b' = BagUnion({b, One(2), One(3)})
        \/ b' = b (-) b
Inv == /\ IsABag(b) \/ b = EmptyBag
       /\ BagCardinality(b) <= 6
       /\ b \sqsubseteq (b (+) One(3))
       /\ ~((b (+) One(5)) \sqsubseteq b)
       /\ BagUnion({b}) = b
       /\ BagUnion({}) = EmptyBag
       /\ ~IsABag([x \in {1} |-> 0]) /\ ~IsABag(3) /\ ~IsABag({1})
       /\ IsABag(<<1, 2>>) /\ BagCardinality(<<1, 2>>) = 3
       /\ <<2, 1>> (-) <<1, 1>> = [x \in {1} |-> 1]
       /\ BagCardinality(b (+) <<1>>) = BagCardinality(b) + 1
====
