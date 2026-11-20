---- MODULE ViewLoop ----
\* x toggles while y counts to 3. Under VIEW x the state x = 0, y = 2 has the
\* same view as the initial state, but y = 3 is reached on every behavior.
EXTENDS Naturals
VARIABLES x, y
vars == <<x, y>>
Init == x = 0 /\ y = 0
Next == x' = 1 - x /\ y' = IF y < 3 THEN y + 1 ELSE y
Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(Next)
EventuallyThree == <>(y = 3)
SafeY == y <= 3
BadY == y < 2
View == x
====
