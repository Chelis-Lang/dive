---- MODULE RecLiveInner ----
EXTENDS Naturals
VARIABLES x, done
Sum[n \in 0..4] == IF n = 0 THEN 0 ELSE Sum[n-1] + n
Init == x = 0 /\ done = FALSE
Step == x < 4 /\ Sum[x] < 10 /\ x' = x + 1 /\ UNCHANGED done
Finish == x = 4 /\ ~done /\ done' = TRUE /\ UNCHANGED x
Next == Step \/ Finish
Fair == WF_<<x, done>>(Step) /\ WF_<<x, done>>(Finish)
Prop == <>(~ENABLED Step) /\ <>[]done
====
