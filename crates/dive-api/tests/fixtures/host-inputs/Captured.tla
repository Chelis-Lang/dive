---- MODULE Captured ----
EXTENDS Naturals, IOUtils
Environment == IOEnv
Payload == Deserialize("input.txt", [format |-> "TXT", charset |-> "UTF-8"])
VARIABLE x
Init == /\ x = atoi(Environment["DIVE_COUNT"])
        /\ Environment["DIVE_LABEL"] = "outside"
        /\ Payload.exitValue = 0
        /\ Payload.stdout = "hello\n"
        /\ Payload.stderr = ""
Next == x' = x + 1
Inv == x < 4
====
