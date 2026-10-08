---- MODULE EvTLC ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME Print("x", 1) = 1
ASSUME Print(<<1, "s">>, {2, 1}) = {1, 2}
ASSUME PrintT(ToString(<<1, "a">>))
ASSUME PrintT(ToString({3, 1}))
ASSUME PrintT(ToString(1..3))
ASSUME PrintT(ToString("q"))
ASSUME PrintT(1 :> 2)
ASSUME PrintT((1 :> 2 @@ 3 :> 4)[3])
ASSUME PrintT(SortSeq(<<3, 1, 2>>, LAMBDA a, b : a < b))
ASSUME PrintT(SortSeq(<<>>, LAMBDA a, b : a < b))
ASSUME PrintT(SortSeq(<<1, 2, 3>>, LAMBDA a, b : a > b))
ASSUME PrintT(Permutations({1, 2, 3}))
ASSUME PrintT(Permutations({}))
ASSUME PrintT(TLCEval(1..3))
ASSUME PrintT(TLCEval(SUBSET {1}))
ASSUME PrintT(Assert(TRUE, "m"))
ASSUME PrintT(ToString(TRUE))
ASSUME PrintT(ToString(<<>>))
ASSUME PrintT(ToString([a |-> 1]))
ASSUME PrintT(Cardinality(Permutations(1..4)))
====
