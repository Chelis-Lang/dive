---- MODULE EvalError ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = <<1, 2>>[x + 1] + 1
====
