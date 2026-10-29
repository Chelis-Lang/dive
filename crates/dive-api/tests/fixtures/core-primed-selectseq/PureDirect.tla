---- MODULE PureDirect ----
EXTENDS Integers, Sequences
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
Prop == [][(SelectSeq(s, Odd))' = IF s = <<1>> THEN <<3>> ELSE <<1>>]_s
====
