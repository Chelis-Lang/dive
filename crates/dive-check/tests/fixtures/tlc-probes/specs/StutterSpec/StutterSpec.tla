---- MODULE StutterSpec ----
\* Probe (with StutterNext): the same machine given as SPECIFICATION
\* Init /\ [][Next]_x, with deadlock checking on. If TLC took [Next]_x as the
\* next-state action, every state would also generate its stuttering step
\* (6 generated, no deadlock). If it takes Next, this matches StutterNext and
\* reports a deadlock at x = 2.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Spec == Init /\ [][Next]_x
====
