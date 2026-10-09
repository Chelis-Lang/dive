---- MODULE P3ActionConstraint ----
\* Probe: a cfg directive some tools do not implement. ACTION_CONSTRAINT prunes
\* the step 4 -> 5, so TLC finds exactly 5 distinct states (0..4). A tool that
\* ignores the directive explores 0..10 (11 states) and still says "no error".
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 10 /\ x' = x + 1
Inv == x <= 10
NotFive == x' # 5
====
