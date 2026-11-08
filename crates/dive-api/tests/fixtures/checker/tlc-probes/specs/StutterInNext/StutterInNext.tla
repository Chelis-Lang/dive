---- MODULE StutterInNext ----
\* Probe (with StutterNext and StutterSpec): the cfg's NEXT is [Next]_x itself.
\* TLC then generates each state's stuttering step as a successor, so x = 2 is
\* not a deadlock and "states generated" counts the stuttering steps:
\* expected 1 + 2 + 2 + 1 = 6 generated, 3 distinct, deadlock check on.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Sq == [Next]_x
====
