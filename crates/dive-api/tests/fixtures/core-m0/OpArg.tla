---- MODULE OpArg ----
\* Operator arguments: top-level operators, LAMBDAs, LET operators that
\* read the caller's locals, parameters passed on, and std operators.
EXTENDS Integers, Sequences
VARIABLES x, s
Twice(F(_), v) == F(F(v))
Map(F(_), q) == [i \in 1..Len(q) |-> F(q[i])]
Pass(G(_, _), a, b) == G(a, b)
Through(F(_), v) == Twice(F, v)
Inc(v) == v + 1
Add(a, b) == a + b
Act(A(_), v) == A(v)
Step(v) == x' = v
Init == x = 0 /\ s = <<1, 2>>
Next == /\ x < 6
        /\ \/ Act(Step, Twice(Inc, x))
           \/ LET k == x + 1
                  Bump(v) == v + k
              IN Act(LAMBDA v : x' = v, Through(Bump, x) - x)
        /\ s' = Map(LAMBDA e : e + Pass(Add, x, 1), s)
Inv == /\ Twice(LAMBDA v : v * 2, 1) = 4
       /\ Len(Map(Inc, s)) = 2
====
