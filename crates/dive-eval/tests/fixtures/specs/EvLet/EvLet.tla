---- MODULE EvLet ----
EXTENDS Integers, Sequences, FiniteSets, TLC
RECURSIVE Fact(_)
Fact(n) == IF n = 0 THEN 1 ELSE n * Fact(n - 1)
Apply(F(_), x) == F(x)
Twice(F(_), x) == F(F(x))
Inc(x) == x + 1
Compose(F(_), G(_), x) == F(G(x))
ASSUME PrintT(LET x == 3 IN x + x)
ASSUME PrintT(LET f(a) == a * 2 IN f(4))
ASSUME PrintT(Fact(6))
ASSUME PrintT(Apply(LAMBDA y : y + 1, 3))
ASSUME PrintT(LET G(a) == a + 10 IN Apply(G, 1))
ASSUME PrintT(Twice(Inc, 5))
ASSUME PrintT(Compose(Inc, LAMBDA z : z * 3, 2))
ASSUME PrintT(LET a == 1 b == a + 1 c == b * 10 IN c)
ASSUME PrintT(\A k \in 1..3 : LET sq == k * k IN sq >= k)
ASSUME PrintT(LET H(a) == LET K(b) == a + b IN K(a) IN H(4))
ASSUME PrintT({LET y == x + 1 IN y : x \in 1..3})
ASSUME PrintT(LET RECURSIVE Sum(_) Sum(s) == IF s = {} THEN 0 ELSE LET e == CHOOSE e \in s : TRUE IN e + Sum(s \ {e}) IN Sum(1..5))
ASSUME PrintT(LET m == 2 IN Apply(LAMBDA q : q * m, 21))
====
