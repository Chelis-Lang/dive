---- MODULE EnvBeforeTextOrder ----
EXTENDS IOUtils
Snapshot == IOEnv
Read == Deserialize("input.txt", [format |-> "TXT", charset |-> "UTF-8"])
Picked == CHOOSE s \in {Read.stdout, Snapshot["DIVE_LABEL"]} : TRUE
VARIABLE x
Init == x = Picked
Next == UNCHANGED x
Inv == x = Read.stdout
====
