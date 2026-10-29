---- MODULE EmptyDemand ----
EXTENDS Integers, Sequences
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
Prop == [][(SelectSeq(<<>>, LAMBDA n : 1 \div 0 > n))' = <<>>]_s
====
