---- MODULE Demand ----
EXTENDS Naturals, TLC, IOUtils
Payload == Deserialize(
    IF PrintT("path") THEN "input.txt" ELSE "unused.txt",
    [format |-> IF PrintT("options") THEN "TXT" ELSE "unused", charset |-> "UTF-8"])
VARIABLE x
Init == x = 0 /\ Payload.stdout = "hello\n"
Next == x' = x + 1
Inv == x < 1
====
