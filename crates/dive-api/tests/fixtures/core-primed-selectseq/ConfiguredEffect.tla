---- MODULE ConfiguredEffect ----
EXTENDS Integers, Sequences, TLC
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
CONSTANT Test(_)
Effect(n) == PrintT(n)
Prop == [][(SelectSeq(s, Test))' = s']_s
====
