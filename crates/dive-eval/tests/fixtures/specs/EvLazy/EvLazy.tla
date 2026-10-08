---- MODULE EvLazy ----
EXTENDS Integers, Sequences, FiniteSets, TLC
F(x) == x + x
G(x) == TRUE
H(x, y) == y
ASSUME F(Print("arg", 1)) = 2
ASSUME G(1 \div 0)
ASSUME LET y == Print("let", 3) IN TRUE
ASSUME LET y == Print("let2", 3) IN y = y
ASSUME H(Print("never", 1), Print("once", 2)) = 2
ASSUME \E x \in {3, 1, 2} : PrintT(<<"e", x>>) /\ FALSE
ASSUME {x \in {3, 1, 2} : PrintT(<<"f", x>>)} = {1, 2, 3}
ASSUME {PrintT(<<"m", x>>) : x \in {3, 1, 2}} = {TRUE}
ASSUME [x \in {3, 1, 2} |-> PrintT(<<"c", x>>)] # <<>>
ASSUME [x \in 1..3 |-> 1 \div (x - 2)][1] = -1
ASSUME \A x \in 1..2, y \in {"a", "b"} : PrintT(<<x, y>>)
ASSUME (CHOOSE x \in {3, 1, 2} : PrintT(<<"ch", x>>) /\ x = 2) = 2
ASSUME \A x \in 1..3 : x = 2 => PrintT(<<"imp", x>>)
ASSUME [x \in 1..2, y \in 3..4 |-> PrintT(<<x, y>>)] # <<>>
ASSUME F(PrintT("twice?")) = FALSE \/ TRUE
ASSUME LET z == PrintT("lz") IN z /\ z
ASSUME {x \in SUBSET {1, 2} : PrintT(<<"sub", x>>)} # {}
ASSUME Cardinality({x \in {y \in Nat : y < 3} : PrintT(<<"nat", x>>)} \cap {1}) = 1
====
