---- MODULE StartupFilterAliasTrue ----
EXTENDS Integers
VARIABLE x
Init==x=0
Next==UNCHANGED x
D=={k \in SUBSET {0}: TRUE}
Alias==D
A==x'=0 /\ D=Alias
Prop==[]<>ENABLED A
====
