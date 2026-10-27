---- MODULE Big ----
\* Integers outside the core's inline 61-bit range (D-004).
EXTENDS Integers
CONSTANT N
VARIABLES x, y
Init == x = 2^62 - 3 /\ y \in {-(2^70), 5, 2^61}
Next == \/ /\ x < 2^62 + 3
           /\ x' = x + 1
           /\ y' = IF y > 2^65 THEN y \div 7 ELSE y * 3 - 1
        \/ /\ x >= 2^62 + 3
           /\ UNCHANGED <<x, y>>
Inv == /\ x \in Nat /\ x \in Int /\ x > 2^61 /\ -x < 0
       /\ x % 5 \in 0..4 /\ (x \div 2) * 2 + (x % 2) = x
       /\ x \notin 0..N /\ y \in Int
       /\ {x, 2^62, x - 1} \subseteq Int
       /\ (y < 0) => (y * y > y)
       /\ 2^62 \in {x, 2^62} /\ x # 2^70 /\ <<x>> # <<2^70>>
       /\ [k \in {2^63} |-> k][2^63] = 2^63
====
