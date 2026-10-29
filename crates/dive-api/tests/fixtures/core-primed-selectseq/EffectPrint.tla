---- MODULE EffectPrint ----
EXTENDS Integers, Sequences, TLC
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
Prop == [][(SelectSeq(s, LAMBDA n : PrintT(n)))' = s']_s
====
