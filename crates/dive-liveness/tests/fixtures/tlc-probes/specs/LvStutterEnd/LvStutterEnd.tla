---- MODULE LvStutterEnd ----
\* x counts to 3 and stops (deadlock checking off). WF_x(Next) allows
\* stuttering once Next is disabled, so <>(x = 5) is violated by 0, 1, 2, 3
\* followed by stuttering. Does TLC report a stuttering ending of length 4?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(x = 5)
====
