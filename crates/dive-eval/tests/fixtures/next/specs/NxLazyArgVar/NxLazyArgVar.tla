---- MODULE NxLazyArgVar ----
\* Probe: an operator argument naming a variable stays lazy, and the
\* enumerator sees through it (getVar/getPrimedVar): F(x) assigns x in Init
\* and G(x) assigns x' in Next.
EXTENDS Naturals
VARIABLE x
F(v) == v \in {1, 2}
G(v) == v' = v + 1 /\ v < 4
Init == F(x)
Next == G(x)
====
