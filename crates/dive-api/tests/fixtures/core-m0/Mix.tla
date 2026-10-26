---- MODULE Mix ----
EXTENDS Naturals, Sequences, FiniteSets
CONSTANT N
VARIABLES q, r, s, f
Vals == 1..N
Rec == [a : Vals, b : BOOLEAN]
RECURSIVE Sum(_)
Sum(S) == IF S = {} THEN 0 ELSE LET x == CHOOSE y \in S : TRUE IN x + Sum(S \ {x})
Init == /\ q = <<>>
        /\ r \in Rec
        /\ s \in SUBSET Vals
        /\ f \in [Vals -> {0, 1}]
Push(v) == /\ Len(q) < 2
           /\ q' = Append(q, v)
           /\ UNCHANGED <<r, s>>
           /\ f' = [f EXCEPT ![v] = 1 - @]
Pop == /\ q /= <<>>
       /\ q' = Tail(q)
       /\ r' = [r EXCEPT !.a = Head(q), !.b = ~@]
       /\ s' \in {t \in SUBSET Vals : Cardinality(t) <= 1}
       /\ UNCHANGED f
Flip == \A v \in Vals : /\ f[v] = 0
                        /\ f' = [w \in Vals |-> 1]
                        /\ UNCHANGED <<q, r, s>>
Next == \/ \E v \in Vals : Push(v)
        \/ Pop
        \/ Flip
        \/ CASE Sum(s) > N -> UNCHANGED <<q, r, s, f>>
             [] OTHER -> FALSE
TypeOK == /\ Len(q) <= 2 /\ r \in Rec /\ s \subseteq Vals
          /\ f \in [Vals -> {0, 1}]
Short == Len(q) < 2 \/ r.b
Bound == Sum(s) <= 2 * N
Spec == Init /\ [][Next]_<<q, r, s, f>>
====
