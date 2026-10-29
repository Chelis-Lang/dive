---- MODULE InstanceState ----
EXTENDS Integers, Sequences
VARIABLE s
Init == s = <<1>>
Next == s' = IF s = <<1>> THEN <<2,3>> ELSE <<1>>
Odd(n) == n % 2 = 1
StateTest(n) == n = Len(s)
M == INSTANCE SelectParam WITH cut <- Len(s), Test <- Odd
Prop == [][M!ByValue' = <<1>>]_s
====
