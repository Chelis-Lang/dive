---- MODULE P1Overflow ----
\* Probe: integer arithmetic beyond 2^31. TLC's integers are 32-bit and
\* overflow is an evaluation error. A tool with wider integers that reports
\* "no error" here has silently diverged from TLC.
EXTENDS Integers
VARIABLE x
Init == x = 2147483646
Next == x < 2147483648 /\ x' = x + 1
Inv == x > 0
====
