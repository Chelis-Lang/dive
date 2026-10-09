---- MODULE P6IntDiv ----
\* Probe: \div and % with negative operands. In TLA+ (Integers), a \div b is
\* floor division and a % b lies in 0..b-1 for b > 0. So -7 \div 2 = -4 and
\* -7 % 2 = 1. Truncating division (Rust's / and %) gives -3 and -1.
EXTENDS Integers
VARIABLE q, r
Init == q = (-7) \div 2 /\ r = (-7) % 2
Next == UNCHANGED <<q, r>>
Inv == q = -4 /\ r = 1
====
