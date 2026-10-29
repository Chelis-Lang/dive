---- MODULE OtherHigherOrder ----
EXTENDS Integers, Sequences, TLC
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
Prop == [][(SortSeq(s, LAMBDA a,b : a < b))' = s']_s
====
