---- MODULE AtoiConcatBeforeEnvOrder ----
EXTENDS TLC, IOUtils, Sequences
Early == atoi("0")
Made == "a" \o "Unknown"
Snapshot == IOEnv
Picked == CHOOSE s \in {Made, Snapshot["DIVE_LABEL"]} : TRUE
VARIABLE x
Init == x = Picked
Next == UNCHANGED x
Inv == x = Made
====
