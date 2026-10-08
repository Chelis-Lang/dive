---- MODULE RndStartup ----
\* TLC's start-up random stream: constant definitions (evaluated at start-up, in
\* definition order), then the ASSUMEs in order, then Init, all drawing from one
\* java.util.Random(seed). Every operator kind is exercised at constant level.
\* The field names zq and aq appear here first, so TLC orders zq before aq.
EXTENDS Integers, TLC, Randomization
C1 == RandomElement(1..1000000)
C2 == RandomSubset(3, 1..100)
ASSUME PrintT(<<"c", C1, C2>>)
ASSUME PrintT(<<"interval", RandomElement(1..10), RandomElement(-5..5), RandomElement(7..7)>>)
ASSUME PrintT(<<"enum", RandomElement({3, 1, 4, 1, 5, 9, 2, 6}), RandomElement({{1, 2}, {3}, {}}), RandomElement({<<1, 2>>, <<2, 1>>, <<1>>})>>)
ASSUME PrintT(<<"subsetof", RandomElement(SUBSET (1..4))>>)
ASSUME PrintT(<<"fcns", RandomElement([1..3 -> 1..5]), RandomElement([{10, 20} -> {TRUE, FALSE}]), RandomElement([{} -> 1..3])>>)
ASSUME PrintT(<<"rcds", RandomElement([zq : 1..9, aq : {7, 8}])>>)
ASSUME PrintT(<<"tuples", RandomElement((1..3) \X (4..6) \X {0})>>)
ASSUME PrintT(<<"rs-interval", RandomSubset(3, 1..10), RandomSubset(10, 1..10), RandomSubset(12, 1..10), RandomSubset(0, 1..10), RandomSubset(2, 1..0)>>)
ASSUME PrintT(<<"rs-enum", RandomSubset(2, {10, 20, 30, 40}), RandomSubset(2, SUBSET (1..3))>>)
ASSUME PrintT(<<"rs-product", RandomSubset(3, [1..3 -> 1..4]), RandomSubset(2, [zq : 1..3, aq : 1..2]), RandomSubset(4, (1..2) \X (1..3)), RandomSubset(2, [1..2 -> {}])>>)
ASSUME PrintT(<<"rs-big", RandomSubset(3, [1..3 -> 1..2000]), RandomSubset(2, (1..100000) \X (1..100000))>>)
ASSUME PrintT(<<"rsos", RandomSetOfSubsets(4, 2, 1..5), RandomSetOfSubsets(3, 0, 1..3), RandomSetOfSubsets(3, 3, 1..3), RandomSetOfSubsets(8, 1, 1..3)>>)
ASSUME PrintT(<<"large", RandomElement(1..1000000), RandomSubset(3, 1..1000000)>>)
VARIABLE x
Init == /\ x \in {RandomElement(1..1000), RandomElement(1..1000), RandomElement(1..1000)}
        /\ PrintT(<<"init", x, RandomElement(1..1000000)>>)
Next == UNCHANGED x
====
