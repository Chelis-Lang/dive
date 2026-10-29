---- MODULE PureBindings ----
EXTENDS Integers, Sequences
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
CONSTANT Test(_)
Filter(P(_), q) == SelectSeq(q, P)
Prop == [][ /\ (Filter(Test, s))' = IF s = <<1>> THEN <<3>> ELSE <<1>>
            /\ (LET Local(n) == n % 2 = 1 IN SelectSeq(s, Local))'
                 = IF s = <<1>> THEN <<3>> ELSE <<1>> ]_s
====
