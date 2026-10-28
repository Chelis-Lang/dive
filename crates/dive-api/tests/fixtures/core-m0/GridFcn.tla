---- MODULE GridFcn ----
\* Functions on pair domains, looked up by position when the domain is a
\* dense rectangle and by search otherwise, and S \ {x} folds
\* (04-plan-compiler.md section 6.1). The invariant fails once the sum of
\* the grid reaches 5.
EXTENDS Naturals
Rect == (0..3) \X (1..3)
Odd == {<<1, 1>>, <<1, 2>>, <<1, 3>>, <<1, 4>>, <<2, 1>>,
        <<2, 2>>, <<3, 1>>, <<3, 2>>, <<3, 3>>}
Tri == {p \in (1..4) \X (1..4) : p[2] <= p[1]}
RECURSIVE Sum(_, _)
Sum(f, S) == IF S = {} THEN 0
             ELSE LET x == CHOOSE x \in S : TRUE IN f[x] + Sum(f, S \ {x})
VARIABLE g
Init == g = [p \in Rect |-> 0]
Next == \E p \in Rect : g[p] < 2 /\ g' = [g EXCEPT ![p] = g[p] + 1]
k == g[<<0, 1>>]
odd == [p \in Odd |-> p[1] * 10 + p[2] + k]
tri == [p \in Tri |-> p[1] + p[2] + k]
Inv == /\ odd[<<2, 1>>] = 21 + k /\ odd[<<1, 4>>] = 14 + k
       /\ odd[<<3, 3>>] = 33 + k
       /\ tri[<<4, 4>>] = 8 + k /\ tri[<<3, 1>>] = 4 + k
       /\ Sum(odd, Odd) = 189 + 9 * k
       /\ ({1, 2, 3, k + 5} \ {2, k + 5}) = {1, 3}
       /\ Sum(g, Rect) < 5
====
