---- MODULE StartupFilterUnreachedError ----
EXTENDS Integers
VARIABLE x
Init==x=0
Next==UNCHANGED x
D=={k \in SUBSET {0}: 1 \div 0=0}
Alias==D
A==FALSE /\ x'=0 /\ D=D
Prop==[]<>ENABLED A
====
