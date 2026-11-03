---- MODULE TextBeforeEnvOrder ----
EXTENDS IOUtils
Read == Deserialize("input.txt", [format |-> "TXT", charset |-> "UTF-8"])
Snapshot == IOEnv
Picked == CHOOSE s \in {Read.stdout, Snapshot["DIVE_LABEL"]} : TRUE
VARIABLE x
Init == x = Picked
Next == UNCHANGED x
Inv == x = Read.stdout
====
