---- MODULE Rec ----
EXTENDS Naturals, Sequences
VARIABLE s
RECURSIVE Sum(_)
Sum(q) == IF q = <<>> THEN 0 ELSE Head(q) + Sum(Tail(q))
RECURSIVE Some(_)
Some(q) == IF q = <<>> THEN FALSE ELSE (\E x \in {Head(q)} : x > 0) \/ Some(Tail(q))
RECURSIVE AllSmall(_)
AllSmall(q) == IF q = <<>> THEN TRUE
               ELSE LET h == Head(q)
                        m == CHOOSE y \in 0..5 : y = h
                    IN m < 3 /\ AllSmall(Tail(q))
RECURSIVE Unbounded(_)
Unbounded(q) == IF q = <<>> THEN TRUE ELSE (CHOOSE x : x = Head(q)) > 0 /\ Unbounded(Tail(q))
RECURSIVE Zero
Zero == IF TRUE THEN Sum(s) >= 0 ELSE Zero
RECURSIVE ValueShapes(_)
ValueShapes(n) == IF n = 0 THEN TRUE
                 ELSE LET f[i \in 0..n] == IF i = 0 THEN 0 ELSE f[i-1]+1
                          g(x) == x + Len(s)
                      IN (\A x \in {g(y) : y \in {f[n]}} : x >= Len(s))
                         /\ ValueShapes(n-1)
RECURSIVE Echo(_,_)
Echo(p,n) == IF n = 0 THEN p ELSE Echo(p,n-1)
Init == s = <<1, 2>>
Dec == /\ s # <<>>
       /\ \E k \in 1..Sum(s) : /\ Sum(s) >= k
                               /\ s' = Tail(s)
Next == Dec \/ (s = <<>> /\ UNCHANGED s)
Spec == Init /\ [][Next]_s /\ WF_s(Dec)
Done == <>(Sum(s) = 0)
Never == <>(Sum(s) = 7)
Enumerates == <>(~Some(s))
Direct == []AllSmall(s)
DirectFalse == <>[]Some(s)
Opaque == <>Unbounded(s)
ZeroProperty == <>Zero
Shapes == []<>ValueShapes(2)
ActionActual == <>[][Echo((s' # s) => Len(s') < Len(s),1)]_s
ActionActualFalse == []<><<Echo(s' # s,1)>>_s
====
