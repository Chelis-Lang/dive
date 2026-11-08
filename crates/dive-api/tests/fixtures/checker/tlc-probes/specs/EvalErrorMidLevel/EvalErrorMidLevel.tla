---- MODULE EvalErrorMidLevel ----
\* Probe: counts when the next-state relation fails to evaluate part-way
\* through a level. Expanding 0 generates 1 then 2. Expanding 1 generates 2
\* (already seen), then action B divides by zero. Expected: 4 generated,
\* 3 distinct, 1 on the queue (state 2).
EXTENDS Integers
VARIABLE x
Init == x = 0
A == x' = x + 1
B == x' = IF x = 1 THEN 1 \div 0 ELSE x + 2
Next == A \/ B
====
