---- MODULE ConfiguredState ----
EXTENDS Integers, Sequences
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
CONSTANT Test(_)
StateTest(n) == n = Len(s)
Prop == [][(SelectSeq(s, Test))' = <<1>>]_s
====
